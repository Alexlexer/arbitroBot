using System.IdentityModel.Tokens.Jwt;
using System.Security.Claims;
using System.Text;
using Microsoft.Data.Sqlite;
using Microsoft.IdentityModel.Tokens;

namespace ArbitHub.Api.Services;

/// <summary>
/// Manages users in a local SQLite database.
/// Passwords are hashed with BCrypt (work factor 12).
/// Successful logins/registrations return a signed JWT.
/// </summary>
public sealed class AuthService
{
    private readonly IConfiguration _config;
    private readonly ILogger<AuthService> _logger;

    private string DbPath => _config["Auth:UsersDbPath"] ?? "data/users.db";

    public AuthService(IConfiguration config, ILogger<AuthService> logger)
    {
        _config = config;
        _logger = logger;
    }

    /// <summary>Create the users table if it doesn't exist. Call once at startup.</summary>
    public void Initialize()
    {
        var dir = Path.GetDirectoryName(DbPath);
        if (!string.IsNullOrEmpty(dir))
            Directory.CreateDirectory(dir);

        using var conn = Open();
        Execute(conn, @"
            CREATE TABLE IF NOT EXISTS users (
                id            INTEGER PRIMARY KEY AUTOINCREMENT,
                username      TEXT    UNIQUE NOT NULL COLLATE NOCASE,
                password_hash TEXT    NOT NULL,
                created_at    INTEGER NOT NULL
            )");

        _logger.LogInformation("User database ready at {Path}", DbPath);
    }

    // ── Login / Register ──────────────────────────────────────────────────────

    public AuthResult Login(string username, string password)
    {
        if (string.IsNullOrWhiteSpace(username) || string.IsNullOrWhiteSpace(password))
            return AuthResult.Fail("Username and password are required");

        using var conn = Open();
        using var cmd  = conn.CreateCommand();
        cmd.CommandText = "SELECT password_hash FROM users WHERE username = @u COLLATE NOCASE LIMIT 1";
        cmd.Parameters.AddWithValue("@u", username.Trim());

        var hash = cmd.ExecuteScalar() as string;

        // Constant-time comparison via BCrypt.Verify; avoids user enumeration
        if (hash is null || !BCrypt.Net.BCrypt.Verify(password, hash))
            return AuthResult.Fail("Invalid username or password");

        _logger.LogInformation("User logged in: {Username}", username);
        return AuthResult.Ok(username.Trim(), IssueToken(username.Trim()));
    }

    public AuthResult Register(string username, string password, string inviteCode)
    {
        // Invite code check
        var expected = _config["Auth:InviteCode"];
        if (string.IsNullOrEmpty(expected) || inviteCode != expected)
            return AuthResult.Fail("Invalid invite code");

        // Input validation
        username = username?.Trim() ?? "";
        if (username.Length < 3 || username.Length > 32)
            return AuthResult.Fail("Username must be 3–32 characters");

        if (password is null || password.Length < 8)
            return AuthResult.Fail("Password must be at least 8 characters");

        // Hash with BCrypt — work factor 12 (≈ 300ms on typical hardware)
        var hash = BCrypt.Net.BCrypt.HashPassword(password, workFactor: 12);

        using var conn = Open();
        using var cmd  = conn.CreateCommand();
        cmd.CommandText = @"
            INSERT INTO users (username, password_hash, created_at)
            VALUES (@u, @h, @t)";
        cmd.Parameters.AddWithValue("@u", username);
        cmd.Parameters.AddWithValue("@h", hash);
        cmd.Parameters.AddWithValue("@t", DateTimeOffset.UtcNow.ToUnixTimeMilliseconds());

        try
        {
            cmd.ExecuteNonQuery();
        }
        catch (SqliteException ex) when (ex.SqliteErrorCode == 19) // SQLITE_CONSTRAINT (UNIQUE)
        {
            return AuthResult.Fail("Username already taken");
        }

        _logger.LogInformation("New user registered: {Username}", username);
        return AuthResult.Ok(username, IssueToken(username));
    }

    // ── JWT ───────────────────────────────────────────────────────────────────

    private string IssueToken(string username)
    {
        var key         = _config["Jwt:Key"]       ?? throw new InvalidOperationException("Jwt:Key missing");
        var issuer      = _config["Jwt:Issuer"]    ?? "arbit-hub";
        var audience    = _config["Jwt:Audience"]  ?? "arbit-hub-dashboard";
        var expiryHours = int.Parse(_config["Jwt:ExpiryHours"] ?? "24");

        var credentials = new SigningCredentials(
            new SymmetricSecurityKey(Encoding.UTF8.GetBytes(key)),
            SecurityAlgorithms.HmacSha256);

        var token = new JwtSecurityToken(
            issuer:             issuer,
            audience:           audience,
            claims:             [new Claim(ClaimTypes.Name, username)],
            notBefore:          DateTime.UtcNow,
            expires:            DateTime.UtcNow.AddHours(expiryHours),
            signingCredentials: credentials);

        return new JwtSecurityTokenHandler().WriteToken(token);
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    private SqliteConnection Open()
    {
        var conn = new SqliteConnection($"Data Source={DbPath}");
        conn.Open();
        return conn;
    }

    private static void Execute(SqliteConnection conn, string sql)
    {
        using var cmd = conn.CreateCommand();
        cmd.CommandText = sql;
        cmd.ExecuteNonQuery();
    }
}

/// <summary>Result of a login or register operation.</summary>
public record AuthResult(bool Success, string? Token, string? Username, string? Error)
{
    public static AuthResult Ok(string username, string token) =>
        new(true, token, username, null);

    public static AuthResult Fail(string error) =>
        new(false, null, null, error);
}
