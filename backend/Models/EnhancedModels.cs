using System.ComponentModel.DataAnnotations;
using System.ComponentModel.DataAnnotations.Schema;

namespace ArbitHub.Api.Models;

#region Enums

public enum UserRole
{
    Viewer = 0,     // Read-only access to dashboard
    Trader = 1,     // Can view and execute trades
    Admin = 2,      // Full system control
    System = 3      // Internal system user (for automated processes)
}

public enum TradeStatus
{
    Pending = 0,
    Executed = 1,
    Failed = 2,
    RolledBack = 3,
    Cancelled = 4
}

public enum AuditAction
{
    Login = 0,
    Logout = 1,
    TradeExecuted = 2,
    TradeFailed = 3,
    ConfigChanged = 4,
    UserCreated = 5,
    UserUpdated = 6,
    UserDeleted = 7,
    ApiKeyCreated = 8,
    ApiKeyRevoked = 9,
    SystemAlert = 10
}

#endregion

#region User and Authentication Models

public class User
{
    [Key]
    public int Id { get; set; }

    [Required]
    [MaxLength(50)]
    public string Username { get; set; } = string.Empty;

    [Required]
    [MaxLength(100)]
    [EmailAddress]
    public string Email { get; set; } = string.Empty;

    [Required]
    public string PasswordHash { get; set; } = string.Empty;

    [Required]
    public UserRole Role { get; set; } = UserRole.Viewer;

    public bool IsActive { get; set; } = true;

    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
    public DateTime? LastLoginAt { get; set; }
    public DateTime? UpdatedAt { get; set; }

    // Navigation properties
    public virtual ICollection<UserSession> Sessions { get; set; } = new List<UserSession>();
    public virtual ICollection<RefreshToken> RefreshTokens { get; set; } = new List<RefreshToken>();
    public virtual ICollection<AuditLog> AuditLogs { get; set; } = new List<AuditLog>();
    public virtual ICollection<ApiKey> ApiKeys { get; set; } = new List<ApiKey>();
}

public class UserSession
{
    [Key]
    public int Id { get; set; }

    [Required]
    public int UserId { get; set; }

    [Required]
    [MaxLength(500)]
    public string SessionToken { get; set; } = string.Empty;

    [Required]
    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;

    [Required]
    public DateTime ExpiresAt { get; set; }

    [MaxLength(45)]
    public string? IpAddress { get; set; }

    [MaxLength(500)]
    public string? UserAgent { get; set; }

    // Navigation property
    [ForeignKey("UserId")]
    public virtual User User { get; set; } = null!;
}

public class RefreshToken
{
    [Key]
    public int Id { get; set; }

    [Required]
    public int UserId { get; set; }

    [Required]
    [MaxLength(500)]
    public string Token { get; set; } = string.Empty;

    [Required]
    public DateTime ExpiresAt { get; set; }

    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
    public bool IsRevoked { get; set; } = false;
    public DateTime? RevokedAt { get; set; }

    [MaxLength(45)]
    public string? IpAddress { get; set; }

    // Navigation property
    [ForeignKey("UserId")]
    public virtual User User { get; set; } = null!;
}

#endregion

#region Trading Models

public class TradeHistory
{
    [Key]
    public int Id { get; set; }

    [Required]
    [MaxLength(100)]
    public string TradeId { get; set; } = string.Empty;

    [Required]
    [MaxLength(20)]
    public string Symbol { get; set; } = string.Empty;

    [Required]
    [MaxLength(20)]
    public string LongExchange { get; set; } = string.Empty;

    [Required]
    [MaxLength(20)]
    public string ShortExchange { get; set; } = string.Empty;

    [Required]
    [Column(TypeName = "decimal(18,8)")]
    public decimal LongPrice { get; set; }

    [Required]
    [Column(TypeName = "decimal(18,8)")]
    public decimal ShortPrice { get; set; }

    [Required]
    [Column(TypeName = "decimal(10,4)")]
    public decimal SpreadPct { get; set; }

    [Required]
    [Column(TypeName = "decimal(18,2)")]
    public decimal VolumeUsdt { get; set; }

    [Required]
    [Column(TypeName = "decimal(18,2)")]
    public decimal ProfitUsdt { get; set; }

    [Required]
    public TradeStatus Status { get; set; }

    public DateTime ExecutedAt { get; set; } = DateTime.UtcNow;
    public DateTime? CompletedAt { get; set; }

    [MaxLength(1000)]
    public string? ErrorMessage { get; set; }

    // Additional metadata
    public decimal? FeesUsdt { get; set; }
    public decimal? SlippagePct { get; set; }
    public string? Notes { get; set; }
}

public class ArbitrageOpportunity
{
    [Key]
    public int Id { get; set; }

    [Required]
    [MaxLength(20)]
    public string Symbol { get; set; } = string.Empty;

    [Required]
    [MaxLength(20)]
    public string LongExchange { get; set; } = string.Empty;

    [Required]
    [MaxLength(20)]
    public string ShortExchange { get; set; } = string.Empty;

    [Required]
    [Column(TypeName = "decimal(18,8)")]
    public decimal LongPrice { get; set; }

    [Required]
    [Column(TypeName = "decimal(18,8)")]
    public decimal ShortPrice { get; set; }

    [Required]
    [Column(TypeName = "decimal(10,4)")]
    public decimal SpreadPct { get; set; }

    [Required]
    [Column(TypeName = "decimal(18,2)")]
    public decimal VolumeUsdt { get; set; }

    [Required]
    [Column(TypeName = "decimal(18,2)")]
    public decimal EstimatedProfitUsdt { get; set; }

    public DateTime DetectedAt { get; set; } = DateTime.UtcNow;
    public DateTime? ExpiresAt { get; set; }

    // Risk metrics
    [Column(TypeName = "decimal(10,4)")]
    public decimal? FundingRateImpactPct { get; set; }

    [Column(TypeName = "decimal(10,4)")]
    public decimal? SlippageRiskPct { get; set; }

    [Column(TypeName = "decimal(5,2)")]
    public decimal? LiquidityScore { get; set; } // 0-100 scale

    public bool WasExecuted { get; set; } = false;
    public int? TradeHistoryId { get; set; } // Link to executed trade
}

public class ExchangeBalance
{
    [Key]
    public int Id { get; set; }

    [Required]
    [MaxLength(20)]
    public string Exchange { get; set; } = string.Empty;

    [Required]
    [MaxLength(10)]
    public string Asset { get; set; } = string.Empty;

    [Required]
    [Column(TypeName = "decimal(18,8)")]
    public decimal TotalBalance { get; set; }

    [Required]
    [Column(TypeName = "decimal(18,8)")]
    public decimal AvailableBalance { get; set; }

    [Required]
    [Column(TypeName = "decimal(18,8)")]
    public decimal LockedBalance { get; set; }

    [Column(TypeName = "decimal(10,4)")]
    public decimal? UsdValue { get; set; }

    public DateTime UpdatedAt { get; set; } = DateTime.UtcNow;

    // Exchange-specific metadata
    [MaxLength(100)]
    public string? AccountType { get; set; } // "spot", "futures", "margin"

    public bool IsActive { get; set; } = true;
}

#endregion

#region Security and Audit Models

public class AuditLog
{
    [Key]
    public int Id { get; set; }

    public int? UserId { get; set; }

    [Required]
    [MaxLength(50)]
    public string Action { get; set; } = string.Empty;

    [MaxLength(50)]
    public string? EntityType { get; set; }

    [MaxLength(100)]
    public string? EntityId { get; set; }

    public string? Details { get; set; }

    [MaxLength(45)]
    public string? IpAddress { get; set; }

    [MaxLength(500)]
    public string? UserAgent { get; set; }

    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;

    // Navigation property
    [ForeignKey("UserId")]
    public virtual User? User { get; set; }
}

public class ApiKey
{
    [Key]
    public int Id { get; set; }

    [Required]
    public int UserId { get; set; }

    [Required]
    [MaxLength(100)]
    public string Name { get; set; } = string.Empty;

    [Required]
    [MaxLength(500)]
    public string KeyHash { get; set; } = string.Empty;

    [MaxLength(500)]
    public string? Permissions { get; set; }

    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
    public DateTime? ExpiresAt { get; set; }
    public DateTime? LastUsedAt { get; set; }

    public bool IsActive { get; set; } = true;
    public bool IsRevoked { get; set; } = false;
    public DateTime? RevokedAt { get; set; }

    // Navigation property
    [ForeignKey("UserId")]
    public virtual User User { get; set; } = null!;
}

public class FailedLoginAttempt
{
    [Key]
    public int Id { get; set; }

    [MaxLength(50)]
    public string? Username { get; set; }

    [MaxLength(45)]
    public string? IpAddress { get; set; }

    public DateTime AttemptTime { get; set; } = DateTime.UtcNow;

    [MaxLength(1000)]
    public string? Reason { get; set; }
}

#endregion

#region System Configuration

public class SystemConfig
{
    [Key]
    public int Id { get; set; }

    [Required]
    [MaxLength(100)]
    public string Key { get; set; } = string.Empty;

    [MaxLength(1000)]
    public string? Value { get; set; }

    [MaxLength(50)]
    public string? DataType { get; set; } // "string", "int", "decimal", "bool", "json"

    [MaxLength(500)]
    public string? Description { get; set; }

    public bool IsEncrypted { get; set; } = false;
    public bool IsReadOnly { get; set; } = false;

    public DateTime CreatedAt { get; set; } = DateTime.UtcNow;
    public DateTime UpdatedAt { get; set; } = DateTime.UtcNow;

    [MaxLength(100)]
    public string? UpdatedBy { get; set; }
}

#endregion

#region DTOs for API Requests/Responses

public class UserCreateRequest
{
    [Required]
    [MinLength(3)]
    [MaxLength(50)]
    public string Username { get; set; } = string.Empty;

    [Required]
    [EmailAddress]
    [MaxLength(100)]
    public string Email { get; set; } = string.Empty;

    [Required]
    [MinLength(8)]
    public string Password { get; set; } = string.Empty;

    [Required]
    public string InviteCode { get; set; } = string.Empty;

    public UserRole? Role { get; set; }
}

public class UserUpdateRequest
{
    [EmailAddress]
    [MaxLength(100)]
    public string? Email { get; set; }

    [MinLength(8)]
    public string? Password { get; set; }

    public UserRole? Role { get; set; }
    public bool? IsActive { get; set; }
}

public class UserResponse
{
    public int Id { get; set; }
    public string Username { get; set; } = string.Empty;
    public string Email { get; set; } = string.Empty;
    public UserRole Role { get; set; }
    public bool IsActive { get; set; }
    public DateTime CreatedAt { get; set; }
    public DateTime? LastLoginAt { get; set; }
}

public class TradeHistoryResponse
{
    public int Id { get; set; }
    public string TradeId { get; set; } = string.Empty;
    public string Symbol { get; set; } = string.Empty;
    public string LongExchange { get; set; } = string.Empty;
    public string ShortExchange { get; set; } = string.Empty;
    public decimal LongPrice { get; set; }
    public decimal ShortPrice { get; set; }
    public decimal SpreadPct { get; set; }
    public decimal VolumeUsdt { get; set; }
    public decimal ProfitUsdt { get; set; }
    public TradeStatus Status { get; set; }
    public DateTime ExecutedAt { get; set; }
    public DateTime? CompletedAt { get; set; }
    public decimal? FeesUsdt { get; set; }
    public decimal? SlippagePct { get; set; }
}

public class ApiKeyCreateRequest
{
    [Required]
    [MaxLength(100)]
    public string Name { get; set; } = string.Empty;

    [MaxLength(500)]
    public string? Permissions { get; set; }

    public DateTime? ExpiresAt { get; set; }
}

public class ApiKeyResponse
{
    public int Id { get; set; }
    public string Name { get; set; } = string.Empty;
    public string? Permissions { get; set; }
    public DateTime CreatedAt { get; set; }
    public DateTime? ExpiresAt { get; set; }
    public DateTime? LastUsedAt { get; set; }
    public bool IsActive { get; set; }
    public string Key { get; set; } = string.Empty; // Only returned on creation
}

#endregion