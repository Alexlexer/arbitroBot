using System.Security.Claims;
using System.Text.Json;
using ArbitHub.Api.Services;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.SignalR;

namespace ArbitHub.Api.Hubs;

/// <summary>
/// Server-to-client SignalR method signatures.
/// </summary>
public interface IArbitHubClient
{
    /// <summary>Full bot state (tickers + opportunities + account). Sent every ~2s by the Rust bot.</summary>
    Task ReceiveState(JsonElement state);

    /// <summary>Bot configuration snapshot. Sent when config changes or on initial connect.</summary>
    Task ReceiveBotConfig(JsonElement config);

    /// <summary>Listener alert (external symbol discovery).</summary>
    Task ReceiveListenerAlert(JsonElement alert);

    /// <summary>Best cross-exchange opportunity for a listener-flagged symbol.</summary>
    Task ReceiveListenerOpportunity(JsonElement opportunity);

    /// <summary>Whether the .NET backend is currently connected to RabbitMQ.</summary>
    Task ReceiveConnectionStatus(bool connected);
}

/// <summary>
/// Main SignalR hub. Requires a valid JWT — obtain one via POST /api/auth/login.
///
/// Connect with:
///   const conn = new HubConnectionBuilder()
///     .withUrl("/hubs/arbit", { accessTokenFactory: () => token })
///     .build();
/// </summary>
[Authorize]
public class ArbitSignalRHub : Hub<IArbitHubClient>
{
    private readonly RabbitMqBridgeService _bridge;
    private readonly ILogger<ArbitSignalRHub> _logger;

    public ArbitSignalRHub(RabbitMqBridgeService bridge, ILogger<ArbitSignalRHub> logger)
    {
        _bridge = bridge;
        _logger = logger;
    }

    public override async Task OnConnectedAsync()
    {
        var username = Context.User?.FindFirst(ClaimTypes.Name)?.Value ?? "anonymous";
        _logger.LogInformation("SignalR connected: {Id} ({User})", Context.ConnectionId, username);

        // Immediately push the current RabbitMQ connection state to the new client
        await Clients.Caller.ReceiveConnectionStatus(_bridge.IsConnected);

        await base.OnConnectedAsync();
    }

    public override Task OnDisconnectedAsync(Exception? exception)
    {
        if (exception is not null)
            _logger.LogWarning(exception, "SignalR client disconnected with error: {Id}", Context.ConnectionId);
        else
            _logger.LogInformation("SignalR disconnected: {Id}", Context.ConnectionId);

        return base.OnDisconnectedAsync(exception);
    }

    /// <summary>
    /// Forward a bot command to RabbitMQ.
    /// Auth commands (dashboard_login / dashboard_register) are rejected — use REST endpoints instead.
    ///
    /// Supported command types:
    ///   toggle_exchange, update_spread, update_depth, update_listener_ws_url,
    ///   update_api_keys, update_fees, UpdateConfig, UpdateSecrets,
    ///   EmergencyStop, Resume
    /// </summary>
    public async Task SendBotCommand(JsonElement command)
    {
        if (!command.TryGetProperty("type", out var typeProp))
        {
            _logger.LogWarning("Received command without 'type' from {Id}", Context.ConnectionId);
            return;
        }

        var type = typeProp.GetString();

        if (type is "dashboard_login" or "dashboard_register")
        {
            _logger.LogWarning("Blocked auth command via SignalR from {Id} — use POST /api/auth/login", Context.ConnectionId);
            return;
        }

        _logger.LogDebug("Forwarding command '{Type}' from {Id}", type, Context.ConnectionId);
        await _bridge.PublishCommandAsync(command);
    }
}
