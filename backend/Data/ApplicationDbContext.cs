using Microsoft.EntityFrameworkCore;
using ArbitHub.Api.Models;

namespace ArbitHub.Api.Data;

public class ApplicationDbContext : DbContext
{
    public ApplicationDbContext(DbContextOptions<ApplicationDbContext> options)
        : base(options)
    {
    }

    // Users and authentication
    public DbSet<User> Users { get; set; }
    public DbSet<UserSession> UserSessions { get; set; }
    public DbSet<RefreshToken> RefreshTokens { get; set; }

    // Trading data
    public DbSet<TradeHistory> TradeHistories { get; set; }
    public DbSet<ArbitrageOpportunity> ArbitrageOpportunities { get; set; }
    public DbSet<ExchangeBalance> ExchangeBalances { get; set; }
    public DbSet<SystemConfig> SystemConfigs { get; set; }

    // Security and auditing
    public DbSet<AuditLog> AuditLogs { get; set; }
    public DbSet<ApiKey> ApiKeys { get; set; }
    public DbSet<FailedLoginAttempt> FailedLoginAttempts { get; set; }

    protected override void OnModelCreating(ModelBuilder modelBuilder)
    {
        base.OnModelCreating(modelBuilder);

        // User configuration
        modelBuilder.Entity<User>(entity =>
        {
            entity.HasIndex(u => u.Username).IsUnique();
            entity.HasIndex(u => u.Email).IsUnique();
            entity.Property(u => u.Username).HasMaxLength(50);
            entity.Property(u => u.Email).HasMaxLength(100);
            entity.Property(u => u.PasswordHash).IsRequired();
            entity.Property(u => u.Role).HasConversion<string>().HasMaxLength(20);
            entity.Property(u => u.CreatedAt).HasDefaultValueSql("CURRENT_TIMESTAMP");
            entity.Property(u => u.IsActive).HasDefaultValue(true);
        });

        // UserSession configuration
        modelBuilder.Entity<UserSession>(entity =>
        {
            entity.HasIndex(s => s.SessionToken).IsUnique();
            entity.HasIndex(s => s.ExpiresAt);
            entity.Property(s => s.SessionToken).HasMaxLength(500);
            entity.Property(s => s.IpAddress).HasMaxLength(45);
            entity.Property(s => s.UserAgent).HasMaxLength(500);
        });

        // TradeHistory configuration
        modelBuilder.Entity<TradeHistory>(entity =>
        {
            entity.HasIndex(t => t.TradeId).IsUnique();
            entity.HasIndex(t => t.ExecutedAt);
            entity.HasIndex(t => t.Symbol);
            entity.HasIndex(t => new { t.LongExchange, t.ShortExchange });
            entity.Property(t => t.TradeId).HasMaxLength(100);
            entity.Property(t => t.Symbol).HasMaxLength(20);
            entity.Property(t => t.LongExchange).HasMaxLength(20);
            entity.Property(t => t.ShortExchange).HasMaxLength(20);
            entity.Property(t => t.Status).HasConversion<string>().HasMaxLength(20);
            entity.Property(t => t.ExecutedAt).HasDefaultValueSql("CURRENT_TIMESTAMP");
        });

        // ArbitrageOpportunity configuration
        modelBuilder.Entity<ArbitrageOpportunity>(entity =>
        {
            entity.HasIndex(o => o.DetectedAt);
            entity.HasIndex(o => o.Symbol);
            entity.Property(o => o.Symbol).HasMaxLength(20);
            entity.Property(o => o.LongExchange).HasMaxLength(20);
            entity.Property(o => o.ShortExchange).HasMaxLength(20);
            entity.Property(o => o.DetectedAt).HasDefaultValueSql("CURRENT_TIMESTAMP");
        });

        // ExchangeBalance configuration
        modelBuilder.Entity<ExchangeBalance>(entity =>
        {
            entity.HasIndex(b => new { b.Exchange, b.Asset });
            entity.HasIndex(b => b.UpdatedAt);
            entity.Property(b => b.Exchange).HasMaxLength(20);
            entity.Property(b => b.Asset).HasMaxLength(10);
            entity.Property(b => b.UpdatedAt).HasDefaultValueSql("CURRENT_TIMESTAMP");
        });

        // AuditLog configuration
        modelBuilder.Entity<AuditLog>(entity =>
        {
            entity.HasIndex(a => a.CreatedAt);
            entity.HasIndex(a => a.UserId);
            entity.HasIndex(a => a.Action);
            entity.Property(a => a.Action).HasMaxLength(50);
            entity.Property(a => a.EntityType).HasMaxLength(50);
            entity.Property(a => a.EntityId).HasMaxLength(100);
            entity.Property(a => a.IpAddress).HasMaxLength(45);
            entity.Property(a => a.UserAgent).HasMaxLength(500);
            entity.Property(a => a.CreatedAt).HasDefaultValueSql("CURRENT_TIMESTAMP");
        });

        // ApiKey configuration
        modelBuilder.Entity<ApiKey>(entity =>
        {
            entity.HasIndex(k => k.KeyHash).IsUnique();
            entity.HasIndex(k => k.UserId);
            entity.Property(k => k.Name).HasMaxLength(100);
            entity.Property(k => k.KeyHash).HasMaxLength(500);
            entity.Property(k => k.Permissions).HasMaxLength(500);
            entity.Property(k => k.CreatedAt).HasDefaultValueSql("CURRENT_TIMESTAMP");
        });

        // FailedLoginAttempt configuration
        modelBuilder.Entity<FailedLoginAttempt>(entity =>
        {
            entity.HasIndex(f => f.Username);
            entity.HasIndex(f => f.IpAddress);
            entity.HasIndex(f => f.AttemptTime);
            entity.Property(f => f.Username).HasMaxLength(50);
            entity.Property(f => f.IpAddress).HasMaxLength(45);
            entity.Property(f => f.AttemptTime).HasDefaultValueSql("CURRENT_TIMESTAMP");
        });

        // SystemConfig configuration
        modelBuilder.Entity<SystemConfig>(entity =>
        {
            entity.HasIndex(c => c.Key).IsUnique();
            entity.Property(c => c.Key).HasMaxLength(100);
            entity.Property(c => c.Value).HasMaxLength(1000);
            entity.Property(c => c.UpdatedAt).HasDefaultValueSql("CURRENT_TIMESTAMP");
        });
    }
}