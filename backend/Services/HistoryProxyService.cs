namespace ArbitHub.Api.Services;

/// <summary>
/// Proxies the history API from the Rust Axum server (default: http://localhost:8080).
/// The Rust bot owns the SQLite snapshots DB; we forward the request rather than
/// opening the DB file directly, which avoids locking issues.
/// </summary>
public sealed class HistoryProxyService
{
    private readonly HttpClient _http;
    private readonly ILogger<HistoryProxyService> _logger;

    public HistoryProxyService(HttpClient http, ILogger<HistoryProxyService> logger)
    {
        _http   = http;
        _logger = logger;
    }

    /// <summary>
    /// Returns the raw JSON from GET /api/history?days={days}, or null if unavailable.
    /// </summary>
    public async Task<string?> GetHistoryAsync(int days, CancellationToken ct = default)
    {
        try
        {
            var response = await _http.GetAsync($"/api/history?days={days}", ct);
            response.EnsureSuccessStatusCode();
            return await response.Content.ReadAsStringAsync(ct);
        }
        catch (HttpRequestException ex)
        {
            _logger.LogError(ex, "History service unreachable at {Base}", _http.BaseAddress);
            return null;
        }
        catch (TaskCanceledException)
        {
            _logger.LogWarning("History request timed out");
            return null;
        }
    }
}
