using System.Text;
using System.Text.Json;
using ArbitHub.Api.Hubs;
using Microsoft.AspNetCore.SignalR;
using RabbitMQ.Client;
using RabbitMQ.Client.Events;

namespace ArbitHub.Api.Services;

/// <summary>
/// Background service that bridges the RabbitMQ arbit_hub exchange to SignalR clients.
///
/// Subscribes to:
///   - arbit_hub exchange (#) for: state, config, listener.alert, listener.opportunity
///   - amq.topic exchange (arbit_hub.#) for individual ticker/account STOMP-style messages
///
/// Publishes to:
///   - arbit_hub exchange, routing key: bot.commands
/// </summary>
public sealed class RabbitMqBridgeService : IHostedService, IDisposable
{
    private readonly IHubContext<ArbitSignalRHub, IArbitHubClient> _hub;
    private readonly ILogger<RabbitMqBridgeService> _logger;
    private readonly IConfiguration _config;

    private IConnection? _connection;
    private IModel? _channel;
    private CancellationTokenSource _cts = new();

    private volatile bool _connected;
    public bool IsConnected => _connected;

    public RabbitMqBridgeService(
        IHubContext<ArbitSignalRHub, IArbitHubClient> hub,
        ILogger<RabbitMqBridgeService> logger,
        IConfiguration config)
    {
        _hub    = hub;
        _logger = logger;
        _config = config;
    }

    // ── IHostedService ────────────────────────────────────────────────────────

    public Task StartAsync(CancellationToken cancellationToken)
    {
        _cts = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        _ = Task.Run(() => ConnectLoopAsync(_cts.Token), _cts.Token);
        return Task.CompletedTask;
    }

    public Task StopAsync(CancellationToken cancellationToken)
    {
        _cts.Cancel();
        return Task.CompletedTask;
    }

    public void Dispose()
    {
        _cts.Dispose();
        CloseChannel();
        CloseConnection();
    }

    // ── Connection loop ───────────────────────────────────────────────────────

    private async Task ConnectLoopAsync(CancellationToken ct)
    {
        var rmq      = _config.GetSection("RabbitMq");
        var host     = rmq["Host"]           ?? "localhost";
        var port     = int.Parse(rmq["Port"] ?? "5672");
        var user     = rmq["Username"]       ?? "guest";
        var pass     = rmq["Password"]       ?? "guest";
        var exchange = rmq["Exchange"]       ?? "arbit_hub";
        var retryMs  = int.Parse(rmq["RetryDelayMs"] ?? "5000");

        while (!ct.IsCancellationRequested)
        {
            try
            {
                _logger.LogInformation("Connecting to RabbitMQ at {Host}:{Port}…", host, port);

                var factory = new ConnectionFactory
                {
                    HostName               = host,
                    Port                   = port,
                    UserName               = user,
                    Password               = pass,
                    DispatchConsumersAsync = true,
                    AutomaticRecoveryEnabled = false // we handle retries ourselves
                };

                _connection = factory.CreateConnection("arbit-hub-backend");
                _channel    = _connection.CreateModel();
                _channel.BasicQos(prefetchSize: 0, prefetchCount: 100, global: false);

                // arbit_hub is a topic exchange created by the Rust bot.
                // Declaring it here is idempotent (same params → no-op).
                _channel.ExchangeDeclare(
                    exchange:    exchange,
                    type:        ExchangeType.Topic,
                    durable:     true,
                    autoDelete:  false);

                // ── Queue 1: arbit_hub exchange, all routing keys ─────────────
                // Catches: state, config, listener.alert, listener.opportunity, bot.commands echo
                var mainQueue = DeclareExclusiveQueue(_channel);
                _channel.QueueBind(mainQueue, exchange, "#");

                // ── Queue 2: amq.topic exchange, arbit_hub.# pattern ──────────
                // Catches STOMP-style: arbit_hub.ticker.BTC, arbit_hub.account.state
                // (Only present if Rust publishes to amq.topic — handled gracefully if not)
                string? topicQueue = null;
                try
                {
                    topicQueue = DeclareExclusiveQueue(_channel);
                    _channel.QueueBind(topicQueue, "amq.topic", "arbit_hub.#");
                }
                catch (Exception ex)
                {
                    _logger.LogDebug(ex, "amq.topic binding skipped (not available or no messages there)");
                    topicQueue = null;
                }

                // ── Consumers ─────────────────────────────────────────────────
                var consumer = new AsyncEventingBasicConsumer(_channel);
                consumer.Received += OnMessageReceivedAsync;

                _channel.BasicConsume(mainQueue, autoAck: true, consumer);
                if (topicQueue is not null)
                    _channel.BasicConsume(topicQueue, autoAck: true, consumer);

                _connection.ConnectionShutdown += OnConnectionShutdown;

                _connected = true;
                _logger.LogInformation("RabbitMQ connected. Listening on exchange '{Exchange}'", exchange);
                await _hub.Clients.All.ReceiveConnectionStatus(true);

                // Block until the connection drops or we are cancelled
                while (!ct.IsCancellationRequested && _connection.IsOpen)
                    await Task.Delay(1_000, ct);
            }
            catch (OperationCanceledException)
            {
                break;
            }
            catch (Exception ex)
            {
                _connected = false;
                _logger.LogError(ex, "RabbitMQ connection error. Retrying in {Ms}ms", retryMs);
                await SafeNotifyDisconnectedAsync();

                try { await Task.Delay(retryMs, ct); }
                catch (OperationCanceledException) { break; }
            }
            finally
            {
                CloseChannel();
                CloseConnection();
            }
        }

        _logger.LogInformation("RabbitMQ bridge stopped");
    }

    private void OnConnectionShutdown(object? sender, ShutdownEventArgs e)
    {
        _connected = false;
        _logger.LogWarning("RabbitMQ connection shutdown: {Reason}", e.ReplyText);
        _ = SafeNotifyDisconnectedAsync();
    }

    // ── Message handler ───────────────────────────────────────────────────────

    private async Task OnMessageReceivedAsync(object sender, BasicDeliverEventArgs ea)
    {
        try
        {
            var key  = ea.RoutingKey;
            var body = Encoding.UTF8.GetString(ea.Body.ToArray());

            if (string.IsNullOrWhiteSpace(body)) return;

            _logger.LogTrace("RabbitMQ ← key={Key} len={Len}", key, body.Length);

            var json = JsonSerializer.Deserialize<JsonElement>(body);

            switch (key)
            {
                // Full bot state (tickers + account + opportunities) every ~2s
                case "state":
                    await _hub.Clients.All.ReceiveState(json);
                    break;

                // Config update
                case "config":
                case "bot.config":
                    await _hub.Clients.All.ReceiveBotConfig(json);
                    break;

                // Listener alerts from external WS (e.g., new token detection)
                case "listener.alert":
                case "arbit_hub.listener.alert":
                    await _hub.Clients.All.ReceiveListenerAlert(json);
                    break;

                // Best opportunity for a listener-alerted symbol
                case "listener.opportunity":
                case "arbit_hub.listener.opportunity":
                    await _hub.Clients.All.ReceiveListenerOpportunity(json);
                    break;

                // STOMP-style individual ticker: arbit_hub.ticker.BTC
                case var k when k.StartsWith("arbit_hub.ticker.", StringComparison.OrdinalIgnoreCase)
                             || k.StartsWith("ticker.", StringComparison.OrdinalIgnoreCase):
                    await _hub.Clients.All.ReceiveState(json);
                    break;

                // STOMP-style account state
                case "arbit_hub.account.state":
                case "account.state":
                    await _hub.Clients.All.ReceiveState(json);
                    break;

                // Dashboard auth replies are no longer needed — auth is REST/JWT
                case "dashboard.auth":
                    break;

                default:
                    _logger.LogDebug("Unhandled routing key '{Key}' — ignored", key);
                    break;
            }
        }
        catch (JsonException jex)
        {
            _logger.LogWarning(jex, "Failed to parse RabbitMQ message body");
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Error processing RabbitMQ message");
        }
    }

    // ── Publishing ────────────────────────────────────────────────────────────

    /// <summary>Publish a bot command to the arbit_hub RabbitMQ exchange.</summary>
    public Task PublishCommandAsync(JsonElement command)
    {
        if (_channel is null || !_channel.IsOpen)
        {
            _logger.LogWarning("Cannot publish command — RabbitMQ channel not open");
            return Task.CompletedTask;
        }

        var exchange = _config["RabbitMq:Exchange"] ?? "arbit_hub";
        var body     = Encoding.UTF8.GetBytes(JsonSerializer.Serialize(command));

        var props = _channel.CreateBasicProperties();
        props.ContentType   = "application/json";
        props.DeliveryMode  = 1; // non-persistent — commands are transient

        _channel.BasicPublish(exchange, "bot.commands", props, body);

        _logger.LogDebug("RabbitMQ → bot.commands: {Type}",
            command.TryGetProperty("type", out var t) ? t.GetString() : "?");

        return Task.CompletedTask;
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private static string DeclareExclusiveQueue(IModel channel) =>
        channel.QueueDeclare(queue: "", durable: false, exclusive: true, autoDelete: true).QueueName;

    private async Task SafeNotifyDisconnectedAsync()
    {
        try { await _hub.Clients.All.ReceiveConnectionStatus(false); }
        catch { /* clients may already be disconnected */ }
    }

    private void CloseChannel()
    {
        try { _channel?.Close(); _channel?.Dispose(); } catch { }
        _channel = null;
    }

    private void CloseConnection()
    {
        try { _connection?.Close(); _connection?.Dispose(); } catch { }
        _connection = null;
    }
}
