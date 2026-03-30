using ArbitHub.Api.Models;
using ArbitHub.Api.Services;
using Microsoft.AspNetCore.Mvc;

namespace ArbitHub.Api.Controllers;

/// <summary>
/// REST authentication endpoints.
/// The JWT token returned here is used for both REST calls (Authorization: Bearer)
/// and the SignalR connection (?access_token=...).
/// </summary>
[ApiController]
[Route("api/auth")]
public class AuthController : ControllerBase
{
    private readonly AuthService _auth;

    public AuthController(AuthService auth) => _auth = auth;

    /// <summary>
    /// Login with username + password.
    /// Returns a JWT bearer token valid for 24 hours (configurable via Jwt:ExpiryHours).
    /// </summary>
    [HttpPost("login")]
    [ProducesResponseType(typeof(AuthResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status401Unauthorized)]
    public IActionResult Login([FromBody] LoginRequest request)
    {
        var result = _auth.Login(request.Username, request.Password);

        if (!result.Success)
            return Unauthorized(new ErrorResponse(result.Error!));

        return Ok(new AuthResponse(result.Token!, result.Username!));
    }

    /// <summary>
    /// Register a new dashboard user.
    /// Requires a valid invite code (set via Auth:InviteCode in appsettings.json or env).
    /// </summary>
    [HttpPost("register")]
    [ProducesResponseType(typeof(AuthResponse), StatusCodes.Status200OK)]
    [ProducesResponseType(typeof(ErrorResponse), StatusCodes.Status400BadRequest)]
    public IActionResult Register([FromBody] RegisterRequest request)
    {
        var result = _auth.Register(request.Username, request.Password, request.InviteCode);

        if (!result.Success)
            return BadRequest(new ErrorResponse(result.Error!));

        return Ok(new AuthResponse(result.Token!, result.Username!));
    }
}
