using System.Text;
using ArbitHub.Api.Data;
using ArbitHub.Api.Hubs;
using ArbitHub.Api.Services;
using Microsoft.AspNetCore.Authentication.JwtBearer;
using Microsoft.EntityFrameworkCore;
using Microsoft.IdentityModel.Tokens;

var builder = WebApplication.CreateBuilder(args);
var config = builder.Configuration;

// ── Core ──────────────────────────────────────────────────────────────────────
builder.Services.AddControllers();
builder.Services.AddSignalR(o =>
{
    o.MaximumReceiveMessageSize = 1_048_576; // 1 MB
    o.EnableDetailedErrors = builder.Environment.IsDevelopment();
});
builder.Services.AddEndpointsApiExplorer();
builder.Services.AddSwaggerGen(c =>
{
    c.SwaggerDoc("v1", new() { Title = "ArbitHub API", Version = "v1" });
    c.AddSecurityDefinition("Bearer", new()
    {
        In = Microsoft.OpenApi.Models.ParameterLocation.Header,
        Description = "Enter: Bearer {token}",
        Name = "Authorization",
        Type = Microsoft.OpenApi.Models.SecuritySchemeType.ApiKey
    });
    c.AddSecurityRequirement(new()
    {
        {
            new() { Reference = new() { Type = Microsoft.OpenApi.Models.ReferenceType.SecurityScheme, Id = "Bearer" } },
            []
        }
    });
});

// ── JWT Auth ──────────────────────────────────────────────────────────────────
var jwtKey = config["Jwt:Key"]
    ?? throw new InvalidOperationException("Jwt:Key is required in appsettings.json");
var jwtIssuer   = config["Jwt:Issuer"]   ?? "arbit-hub";
var jwtAudience = config["Jwt:Audience"] ?? "arbit-hub-dashboard";

builder.Services
    .AddAuthentication(JwtBearerDefaults.AuthenticationScheme)
    .AddJwtBearer(opt =>
    {
        opt.TokenValidationParameters = new TokenValidationParameters
        {
            ValidateIssuerSigningKey = true,
            IssuerSigningKey         = new SymmetricSecurityKey(Encoding.UTF8.GetBytes(jwtKey)),
            ValidateIssuer           = true,
            ValidIssuer              = jwtIssuer,
            ValidateAudience         = true,
            ValidAudience            = jwtAudience,
            ValidateLifetime         = true,
            ClockSkew                = TimeSpan.Zero
        };

        // SignalR sends the token in the query string instead of the Authorization header
        opt.Events = new JwtBearerEvents
        {
            OnMessageReceived = ctx =>
            {
                var token = ctx.Request.Query["access_token"];
                if (!string.IsNullOrEmpty(token) &&
                    ctx.HttpContext.Request.Path.StartsWithSegments("/hubs"))
                    ctx.Token = token;
                return Task.CompletedTask;
            }
        };
    });

builder.Services.AddAuthorization();

// ── CORS ──────────────────────────────────────────────────────────────────────
var corsOrigins = config.GetSection("Cors:Origins").Get<string[]>()
    ?? ["http://localhost:5174", "http://localhost:5173"];

builder.Services.AddCors(o => o.AddDefaultPolicy(p =>
    p.WithOrigins(corsOrigins)
     .AllowAnyHeader()
     .AllowAnyMethod()
     .AllowCredentials())); // AllowCredentials required for SignalR

// ── Database ──────────────────────────────────────────────────────────────────
builder.Services.AddDbContext<ApplicationDbContext>(options =>
    options.UseNpgsql(config.GetConnectionString("DefaultConnection") ?? 
        "Host=localhost;Database=arbit_hub;Username=postgres;Password=postgres;Port=5432"));

// ── App services ──────────────────────────────────────────────────────────────
builder.Services.AddSingleton<AuthService>();

// RabbitMqBridgeService is both IHostedService and a singleton accessed by the hub
builder.Services.AddSingleton<RabbitMqBridgeService>();
builder.Services.AddHostedService(sp => sp.GetRequiredService<RabbitMqBridgeService>());

// HistoryProxyService uses typed HttpClient pointing at the Rust Axum API
builder.Services.AddHttpClient<HistoryProxyService>(client =>
{
    var baseUrl = config["History:BaseUrl"] ?? "http://localhost:8080";
    client.BaseAddress = new Uri(baseUrl);
    client.Timeout = TimeSpan.FromSeconds(15);
});
builder.Services.AddSingleton<HistoryProxyService>();

// ── Build & configure pipeline ────────────────────────────────────────────────
var app = builder.Build();

if (app.Environment.IsDevelopment())
{
    app.UseSwagger();
    app.UseSwaggerUI();
}

app.UseCors();
app.UseAuthentication();
app.UseAuthorization();
app.MapControllers();
app.MapHub<ArbitSignalRHub>("/hubs/arbit");
app.MapGet("/health", () => Results.Ok(new { status = "ok" })).AllowAnonymous();

// Initialise the SQLite user database before accepting requests
app.Services.GetRequiredService<AuthService>().Initialize();

app.Run();
