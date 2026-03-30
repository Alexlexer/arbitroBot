# ArbitHub - Multi-Exchange Arbitrage Trading Platform

## 🚀 Overview

ArbitHub is a sophisticated multi-exchange arbitrage trading platform that detects and executes cross-exchange arbitrage opportunities in real-time. The system consists of three main components:

1. **Rust Arbitrage Bot** - High-performance trading engine
2. **.NET Backend API** - PostgreSQL-based backend with JWT authentication
3. **React Dashboard** - Real-time monitoring and control interface

## 📁 Project Structure

```
arbit_hub/
├── backend/                    # .NET Backend + React Dashboard
│   ├── Controllers/           # API Controllers
│   ├── Data/                  # Entity Framework Core (PostgreSQL)
│   ├── Models/                # Data Models
│   ├── Services/              # Business Logic Services
│   ├── Hubs/                  # SignalR Hubs
│   ├── dashboard/             # React Frontend
│   │   ├── src/              # React components
│   │   ├── public/           # Static assets
│   │   └── package.json      # Frontend dependencies
│   ├── Program.cs            # .NET Startup
│   ├── appsettings.json      # Backend configuration
│   └── Dockerfile            # Backend Docker image
│
├── bot/                       # Rust Arbitrage Bot
│   ├── src/                  # Rust source code
│   │   ├── exchange/         # Exchange integrations
│   │   ├── aggregator.rs     # Arbitrage logic
│   │   ├── execution.rs      # Trade execution
│   │   ├── risk_manager.rs   # Risk management
│   │   └── main.rs           # Bot entry point
│   ├── Cargo.toml            # Rust dependencies
│   ├── config.json           # Bot configuration
│   ├── .env                  # Environment variables
│   └── Dockerfile            # Bot Docker image
│
├── deploy/                    # Deployment scripts
│   ├── postgres-init.sql     # PostgreSQL initialization
│   └── rabbitmq.conf         # RabbitMQ configuration
│
├── docs/                      # Documentation
│   ├── OPERATION.md          # Operation guide
│   ├── POSTGRES_SETUP.md     # PostgreSQL setup
│   ├── SECURITY.md           # Security guidelines
│   └── API_REFERENCE.md      # API documentation
│
├── docker-compose.yml         # Docker Compose configuration
├── .gitignore                # Git ignore rules
├── .dockerignore             # Docker ignore rules
└── README.md                 # This file
```

## 🏗️ Architecture

### System Components

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   React         │    │   .NET          │    │   Rust          │
│   Dashboard     │◄──►│   Backend API   │◄──►│   Arbitrage Bot │
│   (Port 5174)   │    │   (Port 5000)   │    │   (Port 9180)   │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         ▼                       ▼                       ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Infrastructure Services                      │
├─────────────────┬─────────────────┬─────────────────────────────┤
│   PostgreSQL    │   RabbitMQ      │   Exchange APIs             │
│   (Port 5432)   │   (Port 5672)   │   (Binance, Bybit, etc.)    │
└─────────────────┴─────────────────┴─────────────────────────────┘
```

### Data Flow

1. **Price Discovery**: Rust bot connects to 8+ exchange WebSockets
2. **Arbitrage Detection**: Real-time spread calculation across exchanges
3. **Risk Validation**: 7-stage risk management pipeline
4. **Execution**: Simulated or live trading with rollback protection
5. **Monitoring**: Real-time updates to dashboard via SignalR
6. **Storage**: Trade history and audit logs in PostgreSQL

## 🛠️ Quick Start

### Prerequisites

- Docker & Docker Compose
- Git
- (Optional) .NET 8 SDK, Node.js 18+, Rust 1.70+

### Local Development

```bash
# Clone the repository
git clone <repository-url>
cd arbit_hub

# Start all services
docker-compose up -d

# Check services
docker-compose ps

# View logs
docker-compose logs -f arbitro-bot
```

### Access Services

- **Dashboard**: http://localhost:5174
- **Backend API**: http://localhost:5000/swagger
- **RabbitMQ Management**: http://localhost:15672 (guest/guest)
- **Bot Health**: http://localhost:9180/health
- **Database Test**: http://localhost:5000/api/test/database

## 🔧 Configuration

### Environment Variables

Create `.env` file in `bot/` directory:

```bash
# Exchange API Keys (optional for monitoring)
BINANCE_API_KEY=your_key
BINANCE_API_SECRET=your_secret
BYBIT_API_KEY=your_key
BYBIT_API_SECRET=your_secret

# Telegram (optional)
TELEGRAM_BOT_TOKEN=your_token
TELEGRAM_CHAT_ID=your_chat_id
BOT_PASSWORD=your_password

# Dashboard
INVITE_CODE=your_invite_code
ENABLE_LIVE_TRADING=false
```

### Bot Configuration (`bot/config.json`)

```json
{
  "min_spread_threshold": 5,
  "depth_usdt": 1000,
  "margin_threshold_low": 0.4,
  "margin_threshold_high": 0.7,
  "polling_interval_ms": 5000
}
```

### Backend Configuration (`backend/appsettings.json`)

```json
{
  "ConnectionStrings": {
    "DefaultConnection": "Host=postgres;Database=arbit_hub;Username=postgres;Password=postgres;Port=5432"
  },
  "Jwt": {
    "Key": "64_char_random_secret_key",
    "Issuer": "arbit-hub",
    "Audience": "arbit-hub-dashboard"
  }
}
```

## 🚀 Deployment

### Production Deployment

1. **Update Secrets**:
   - Generate strong JWT key (64+ characters)
   - Set secure PostgreSQL password
   - Use production exchange API keys
   - Set unique invite code

2. **Docker Compose Production**:
   ```bash
   # Create .env.production
   POSTGRES_PASSWORD=strong_password
   JWT_KEY=64_char_random_secret
   INVITE_CODE=production_invite
   
   # Deploy
   docker-compose -f docker-compose.yml --env-file .env.production up -d
   ```

3. **Monitoring Setup**:
   - Configure log rotation
   - Set up backups for PostgreSQL
   - Monitor disk space and memory
   - Set up alerts for critical errors

## 📊 API Reference

### Authentication

```http
POST /api/auth/login
Content-Type: application/json

{
  "username": "admin",
  "password": "password"
}
```

```http
POST /api/auth/register
Content-Type: application/json

{
  "username": "newuser",
  "password": "password123",
  "inviteCode": "your_invite_code"
}
```

### Real-time Updates

Connect to SignalR hub:
```javascript
const connection = new HubConnectionBuilder()
  .withUrl("/hubs/arbit", { accessToken: "jwt_token" })
  .build();
```

### Trade History

```http
GET /api/history?days=30
Authorization: Bearer {token}
```

## 🔒 Security

### Authentication & Authorization
- JWT tokens with 24-hour expiration
- Role-based access control (Viewer, Trader, Admin)
- BCrypt password hashing (work factor 12)
- Session management with IP tracking

### Audit Logging
- All authentication events logged
- Failed login attempt tracking
- Trade execution records
- 90-day retention policy

### Network Security
- CORS configured for dashboard only
- HTTPS enforced in production
- Rate limiting on authentication endpoints
- IP-based brute force protection

## 🐛 Troubleshooting

### Common Issues

1. **Database Connection Failed**
   ```bash
   # Test database connection
   curl http://localhost:5000/api/test/database
   
   # Check PostgreSQL logs
   docker-compose logs postgres
   ```

2. **RabbitMQ Connection Issues**
   ```bash
   # Check RabbitMQ status
   docker-compose logs rabbitmq
   
   # Test management UI
   # http://localhost:15672 (guest/guest)
   ```

3. **Bot Not Starting**
   ```bash
   # Check bot logs
   docker-compose logs arbitro-bot
   
   # Test bot health
   curl http://localhost:9180/health
   ```

4. **Dashboard Not Connecting**
   ```bash
   # Check backend logs
   docker-compose logs arbitro-backend
   
   # Verify SignalR connection
   # Check browser console for WebSocket errors
   ```

### Logs Location

- **Docker Logs**: `docker-compose logs -f [service]`
- **Application Logs**: Container stdout/stderr
- **Database Logs**: PostgreSQL container logs
- **Audit Logs**: PostgreSQL `audit_logs` table

## 📈 Monitoring & Maintenance

### Health Checks

```bash
# Database health
curl http://localhost:5000/api/test/database

# Backend health
curl http://localhost:5000/health

# Bot health
curl http://localhost:9180/health
```

### Database Maintenance

```bash
# Backup PostgreSQL
docker exec arbitro-postgres pg_dump -U postgres arbit_hub > backup.sql

# Restore from backup
cat backup.sql | docker exec -i arbitro-postgres psql -U postgres arbit_hub

# Vacuum database
docker exec arbitro-postgres psql -U postgres -d arbit_hub -c "VACUUM ANALYZE;"
```

### Performance Monitoring

1. **Database Performance**:
   - Monitor query performance
   - Check index usage
   - Track connection count

2. **Bot Performance**:
   - Monitor WebSocket reconnections
   - Track arbitrage detection rate
   - Check execution latency

3. **System Resources**:
   - CPU/Memory usage
   - Disk I/O
   - Network bandwidth

## 🤝 Contributing

### Development Workflow

1. **Fork the repository**
2. **Create feature branch**
   ```bash
   git checkout -b feature/amazing-feature
   ```
3. **Make changes**
4. **Test thoroughly**
   ```bash
   # Run tests
   cd backend && dotnet test
   cd bot && cargo test
   ```
5. **Commit changes**
   ```bash
   git commit -m "Add amazing feature"
   ```
6. **Push to branch**
   ```bash
   git push origin feature/amazing-feature
   ```
7. **Create Pull Request**

### Code Standards

- **Rust**: Follow Rustfmt and Clippy guidelines
- **C#**: Follow .NET coding conventions
- **JavaScript/React**: Use ESLint and Prettier
- **Documentation**: Update relevant docs for changes
- **Testing**: Write unit tests for new functionality

## 📄 License

Proprietary - All rights reserved.

## 📞 Support

For issues and questions:

1. **Check Documentation**: Review `docs/` directory
2. **Examine Logs**: Use `docker-compose logs`
3. **Test Connections**: Use provided test endpoints
4. **Search Issues**: Check for similar problems

### Emergency Contacts

- **System Admin**: Database and infrastructure issues
- **Trading Team**: Exchange connectivity and execution
- **Development Team**: Code and feature requests

---

**Last Updated**: March 30, 2026  
**Version**: 2.0.0 (PostgreSQL Migration)  
**Status**: Production Ready