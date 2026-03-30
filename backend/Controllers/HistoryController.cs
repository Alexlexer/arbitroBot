using ArbitHub.Api.Models;
using ArbitHub.Api.Services;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;

namespace ArbitHub.Api.Controllers;

/// <summary>
/// Proxies the arbitrage snapshot history from the Rust Axum API.
/// Requires a valid JWT (same token used for SignalR).
/// </summary>
[ApiController]
[Route("api/history")]
[Authorize]
public class HistoryController : ControllerBase
{
    private readonly HistoryProxyService _history;

    public HistoryController(HistoryProxyService history) => _history = history;

    /// <summary>
    /// Returns historical arbitrage snapshots for the past N days.
    /// Snapshots are recorded by the Rust bot every 5 minutes.
    ///
    /// Response format:
    /// [{ timestamp, opportunities: [{ symbol, long_exchange, long_price, short_exchange, short_price, spread }] }]
    /// </summary>
    [HttpGet]
    [ProducesResponseType(StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status502BadGateway)]
    public async Task<IActionResult> GetHistory(
        [FromQuery] int days = 30,
        CancellationToken ct = default)
    {
        if (days is < 1 or > 365)
            return BadRequest(new ErrorResponse("days must be between 1 and 365"));

        var json = await _history.GetHistoryAsync(days, ct);

        if (json is null)
            return StatusCode(StatusCodes.Status502BadGateway,
                new ErrorResponse("History service unavailable — ensure the Rust bot is running"));

        // Pass through the raw JSON so we don't double-allocate the (potentially large) response
        return Content(json, "application/json");
    }
}
