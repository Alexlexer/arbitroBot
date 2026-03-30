using ArbitHub.Api.Data;
using Microsoft.AspNetCore.Authorization;
using Microsoft.AspNetCore.Mvc;
using Microsoft.EntityFrameworkCore;

namespace ArbitHub.Api.Controllers;

[ApiController]
[Route("api/test")]
public class TestController : ControllerBase
{
    private readonly ApplicationDbContext _context;
    private readonly ILogger<TestController> _logger;

    public TestController(ApplicationDbContext context, ILogger<TestController> logger)
    {
        _context = context;
        _logger = logger;
    }

    [HttpGet("database")]
    [AllowAnonymous]
    public async Task<IActionResult> TestDatabase()
    {
        try
        {
            // Test database connection
            var canConnect = await _context.Database.CanConnectAsync();
            
            if (!canConnect)
                return StatusCode(500, new { 
                    status = "error", 
                    message = "Cannot connect to database" 
                });

            // Get database info
            var dbName = _context.Database.GetDbConnection().Database;
            var dataSource = _context.Database.GetDbConnection().DataSource;
            var provider = _context.Database.ProviderName;

            return Ok(new
            {
                status = "ok",
                database = new
                {
                    name = dbName,
                    dataSource,
                    provider,
                    canConnect = true
                }
            });
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Database test failed");
            return StatusCode(500, new { 
                status = "error", 
                message = ex.Message,
                details = ex.ToString()
            });
        }
    }

    [HttpGet("migrations")]
    [AllowAnonymous]
    public async Task<IActionResult> GetMigrations()
    {
        try
        {
            // For simplicity, just check if we can connect and get basic info
            var canConnect = await _context.Database.CanConnectAsync();
            
            if (!canConnect)
                return Ok(new
                {
                    status = "warning",
                    message = "Cannot connect to database to check migrations"
                });

            // Get database provider info
            var provider = _context.Database.ProviderName;
            
            return Ok(new
            {
                status = "ok",
                provider,
                canConnect
            });
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to get migrations");
            return StatusCode(500, new { 
                status = "error", 
                message = ex.Message 
            });
        }
    }

    [HttpPost("migrate")]
    [Authorize(Roles = "Admin")]
    public async Task<IActionResult> ApplyMigrations()
    {
        try
        {
            await _context.Database.MigrateAsync();
            
            _logger.LogInformation("Database migrations applied successfully");
            
            return Ok(new
            {
                status = "ok",
                message = "Migrations applied successfully"
            });
        }
        catch (Exception ex)
        {
            _logger.LogError(ex, "Failed to apply migrations");
            return StatusCode(500, new { 
                status = "error", 
                message = ex.Message 
            });
        }
    }
}