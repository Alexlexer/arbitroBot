using System.ComponentModel.DataAnnotations;

namespace ArbitHub.Api.Models;

public record LoginRequest(
    [Required, MinLength(3), MaxLength(32)] string Username,
    [Required, MinLength(1)]                string Password
);

public record RegisterRequest(
    [Required, MinLength(3), MaxLength(32)] string Username,
    [Required, MinLength(8)]                string Password,
    [Required]                              string InviteCode
);

public record AuthResponse(string Token, string Username);

public record ErrorResponse(string Error);
