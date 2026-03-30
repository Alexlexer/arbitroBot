# ArbitHub Project Structure Documentation

## 📁 Complete Directory Tree

```
arbit_hub/
├── 📁 backend/                    # .NET 8 Backend + React Dashboard
│   ├── 📁 Controllers/           # API Controllers (REST endpoints)
│   │   ├── AuthController.cs     # Authentication endpoints
│   │   ├── HistoryController.cs  # Trade history proxy
│   │   └── TestController.cs     # Database testing
│   │
│   ├── 📁 Data/                  # Entity Framework Core
│   │   ├── ApplicationDbContext.cs       # Database context
│   │   └── Migrations/                   # Database migrations
│   │       └── [timestamp]_InitialCreate.cs
│   │
│   ├── 📁 Models/                # Data Models (C# classes)
│   │   ├── AuthModels.cs        # Authentication DTOs
│   │   └── EnhancedModels.cs    # Enhanced PostgreSQL models
│   │
│   ├── 📁 Services/              # Business Logic Services
│   │   ├── AuthService.cs       # SQLite auth (legacy)
│   │   ├── RabbitMqBridgeService.cs # RabbitMQ ↔ SignalR bridge
│   │   └── HistoryProxyService.cs    # History API proxy
│   │
│   ├── 📁 Hubs/                  # SignalR Hubs (WebSocket)
│   │   └── ArbitSignalRHub.cs   # Real-time communication
│   │
│   ├── 📁 dashboard/             # React 18 Frontend
│   │   ├── 📁 src/              # React source code
│   │   │   ├── 📁 components/   # React components
│   │   │   │   ├── HistoryView.jsx
│   │   │   │   ├── Login.jsx
│   │   │   │   ├── Settings.jsx
│   │   │   │   └── [others].jsx
│   │   │   ├── 📁 hooks/        # Custom React hooks
│   │   │   │   └── useSignalR.js
│   │   │   └── main.jsx         # App entry point
│   │   │
│   │   ├── 📁 public/           # Static assets
│   │   ├── package.json         # Node.js dependencies
│   │   ├── vite.config.js       # Build configuration
│   │   ├── Dockerfile           # Frontend Docker image
│   │   └── nginx.conf           # Nginx configuration
│   │
│   ├── 📄 Program.cs            # .NET Startup & Configuration
│   ├── 📄 appsettings.json      # Application configuration
│   ├── 📄 ArbitHub.Api.csproj   # .NET project file
│   ├── 📄 Dockerfile            # Backend Docker image
│   └── 📄 POSTGRES_SETUP.md     # PostgreSQL setup guide
│
├── 📁 bot/                       # Rust Arbitrage Bot
│   ├── 📁 src/                  # Rust source code
│   │   ├── 📁 exchange/         # Exchange integrations
│   │   │   ├── mod.rs          # Exchange module
│   │   │   ├── binance.rs      # Binance WebSocket
│   │   │   ├── bybit.rs        # Bybit WebSocket
│   │   │   ├── bitget.rs       # Bitget WebSocket
│   │   │   ├── mexc.rs         # MEXC WebSocket
│   │   │   ├── bitmart.rs      # Bitmart WebSocket
│   │   │   ├── kraken.rs       # Kraken WebSocket
│   │   │   └── gate.rs         # Gate.io WebSocket
│   │   │
│   │   ├── aggregator.rs       # Arbitrage detection logic
│   │   ├── execution.rs        # Trade execution engine
│   │   ├── risk_manager.rs     # 7-stage risk validation
│   │   ├── poller.rs           # Data polling service
│   │   ├── messaging.rs        # RabbitMQ communication
│   │   ├── config.rs           # Configuration management
│   │   ├── model.rs            # Data structures
│   │   ├── constants.rs        # Constants and defaults
│   │   └── main.rs             # Bot entry point
│   │
│   ├── 📁 data/                 # Local data storage
│   │   └── arbitro_history.db  # SQLite history (legacy)
│   │
│   ├── 📄 Cargo.toml           # Rust dependencies
│   ├── 📄 Cargo.lock           # Dependency lock file
│   ├── 📄 Dockerfile           # Bot Docker image
│   ├── 📄 config.json          # Bot configuration
│   ├── 📄 .env                 # Environment variables
│   ├── 📄 .env.example         # Environment template
│   └── 📄 secrets.json         # Encrypted secrets
│
├── 📁 deploy/                    # Deployment Configuration
│   ├── 📄 postgres-init.sql    # PostgreSQL initialization
│   ├── 📄 rabbitmq.conf        # RabbitMQ configuration
│   └── 📄 nginx-arbitrobot.conf # Nginx reverse proxy
│
├── 📁 docs/                      # Documentation
│   ├── 📄 README.md            # Main documentation
│   ├── 📄 PROJECT_STRUCTURE.md # This file
│   ├── 📄 OPERATION.md         # Operation checklist
│   ├── 📄 POSTGRES_SETUP.md    # PostgreSQL setup guide
│   ├── 📄 SECURITY.md          # Security guidelines
│   ├── 📄 DEV_PLAN.md          # Development roadmap
│   ├── 📄 MATH_REVIEW.md       # Mathematical formulas
│   └── 📄 LISTENER_INTEGRATION.md # External listener integration
│
├── 📁 .github/                   # GitHub Actions
│   └── 📁 workflows/           # CI/CD pipelines
│
├── 📄 docker-compose.yml        # Docker Compose configuration
├── 📄 .gitignore               # Git ignore rules
├── 📄 .dockerignore            # Docker ignore rules
├── 📄 run-docker.sh            # Docker startup script
└── 📄 README.md                # Project overview
```

## 🔗 Component Relationships

### Backend (backend/)
```
┌─────────────────────────────────────────────────────────────┐
│                    .NET Backend API                         │
├──────────────┬──────────────┬───────────────────────────────┤
│   Controllers│   Services   │        SignalR Hub           │
│   (REST API) │  (Business   │     (Real-time updates)      │
│              │   Logic)     │                               │
└──────┬───────┴──────┬───────┴───────────────┬───────────────┘
       │              │                       │
       ▼              ▼                       ▼
┌──────────────┐┌──────────────┐┌─────────────────────────────┐
│  PostgreSQL  ││  RabbitMQ    ││   React Dashboard          │
│  (Database)  ││  (Messaging) ││   (Frontend UI)            │
└──────────────┘└──────────────┘└─────────────────────────────┘
```

### Bot (bot/)
```
┌─────────────────────────────────────────────────────────────┐
│                    Rust Arbitrage Bot                       │
├──────────────┬──────────────┬───────────────────────────────┤
│   Exchange   │  Aggregator  │      Risk Manager            │
│  WebSockets  │ (Arbitrage   │   (7-stage validation)       │
│              │  Detection)  │                               │
└──────┬───────┴──────┬───────┴───────────────┬───────────────┘
       │              │                       │
       ▼              ▼                       ▼
┌──────────────┐┌──────────────┐┌─────────────────────────────┐
│   Exchange   ││   RabbitMQ   ││     Execution Engine       │
│     APIs     ││  (Messaging) ││  (Trade execution)         │
└──────────────┘└──────────────┘└─────────────────────────────┘
```

## 📊 Data Flow Diagram

```
┌─────────────┐    WebSocket    ┌─────────────┐    REST API    ┌─────────────┐
│   Exchange  │ ──────────────► │   Rust Bot  │ ─────────────► │  .NET API   │
│    APIs     │   Price Data    │             │   Trade Data   │             │
└─────────────┘                 └──────┬──────┘                └──────┬──────┘
                                       │                              │
                                       │ RabbitMQ                     │ SignalR
                                       │ Messaging                    │ WebSocket
                                       ▼                              ▼
                                ┌─────────────┐               ┌─────────────┐
                                │  RabbitMQ   │               │   React     │
                                │   Broker    │               │  Dashboard  │
                                └─────────────┘               └─────────────┘
                                       │                              │
                                       ▼                              │
                                ┌─────────────┐                      │
                                │ PostgreSQL  │ ◄────────────────────┘
                                │  Database   │      Audit Logs
                                └─────────────┘
```

## 🛠️ Build & Deployment Files

### Docker Configuration

| File | Purpose | Location |
|------|---------|----------|
| `Dockerfile` | Rust bot image | `bot/Dockerfile` |
| `Dockerfile` | .NET backend image | `backend/Dockerfile` |
| `Dockerfile` | React dashboard image | `backend/dashboard/Dockerfile` |
| `docker-compose.yml` | Multi-service orchestration | Root |
| `.dockerignore` | Docker ignore patterns | Root |

### Configuration Files

| File | Purpose | Location |
|------|---------|----------|
| `appsettings.json` | .NET backend config | `backend/` |
| `config.json` | Rust bot config | `bot/` |
| `.env` | Environment variables | `bot/` |
| `Cargo.toml` | Rust dependencies | `bot/` |
| `package.json` | Node.js dependencies | `backend/dashboard/` |

## 🔄 Development Workflows

### Backend Development (.NET)
```bash
cd backend
dotnet restore    # Restore packages
dotnet build      # Build solution
dotnet run        # Run locally
dotnet test       # Run tests
dotnet ef migrations add [name]  # Create migration
dotnet ef database update        # Apply migrations
```

### Frontend Development (React)
```bash
cd backend/dashboard
npm install       # Install dependencies
npm run dev       # Development server
npm run build     # Production build
npm run lint      # Code linting
```

### Bot Development (Rust)
```bash
cd bot
cargo build       # Build project
cargo run         # Run locally
cargo test        # Run tests
cargo check       # Check compilation
cargo fmt         # Format code
cargo clippy      # Lint code
```

### Docker Development
```bash
# Build all services
docker-compose build

# Start all services
docker-compose up -d

# View logs
docker-compose logs -f

# Stop services
docker-compose down

# Rebuild and restart
docker-compose up -d --build
```

## 📁 Directory Purpose Details

### backend/ - .NET Backend System
- **Controllers/**: REST API endpoints for client communication
- **Data/**: Entity Framework Core database layer (PostgreSQL)
- **Models/**: Data transfer objects and entity models
- **Services/**: Business logic and external service integration
- **Hubs/**: SignalR WebSocket hubs for real-time updates
- **dashboard/**: React frontend application

### bot/ - Rust Trading Engine
- **src/exchange/**: Exchange-specific WebSocket implementations
- **src/aggregator.rs**: Core arbitrage detection algorithm
- **src/execution.rs**: Trade execution with rollback protection
- **src/risk_manager.rs**: Multi-stage risk validation pipeline
- **src/messaging.rs**: RabbitMQ communication layer
- **data/**: Local storage for history and cache

### deploy/ - Infrastructure
- **postgres-init.sql**: Database initialization scripts
- **rabbitmq.conf**: Message broker configuration
- **nginx-arbitrobot.conf**: Web server configuration

### docs/ - Documentation
- **README.md**: Project overview and quick start
- **PROJECT_STRUCTURE.md**: This detailed structure guide
- **OPERATION.md**: Operational procedures and checklist
- **POSTGRES_SETUP.md**: Database setup and migration guide
- **SECURITY.md**: Security policies and best practices

## 🔧 Environment Setup

### Required Tools
- **Docker & Docker Compose**: Container orchestration
- **.NET 8 SDK**: Backend development
- **Node.js 18+ & npm**: Frontend development
- **Rust 1.70+**: Bot development
- **PostgreSQL 16**: Database (via Docker)
- **RabbitMQ**: Message broker (via Docker)

### Development Environment Variables
```bash
# bot/.env
BINANCE_API_KEY=your_key
BINANCE_API_SECRET=your_secret
TELEGRAM_BOT_TOKEN=your_token
INVITE_CODE=development_invite

# backend/appsettings.json (Development)
"ConnectionStrings": {
  "DefaultConnection": "Host=localhost;Database=arbit_hub_dev;Username=postgres;Password=postgres;Port=5432"
}
```

## 🚀 Deployment Architecture

### Production Stack
```
┌─────────────────────────────────────────────────────────────┐
│                    Load Balancer (Nginx)                    │
├─────────────────────────────────────────────────────────────┤
│   React Dashboard  │   .NET Backend API   │   Static Assets │
├─────────────────────────────────────────────────────────────┤
│              PostgreSQL Cluster (Primary + Replica)         │
├─────────────────────────────────────────────────────────────┤
│                  RabbitMQ Cluster (3 nodes)                 │
├─────────────────────────────────────────────────────────────┤
│              Rust Bot Instances (Auto-scaling)              │
└─────────────────────────────────────────────────────────────┘
```

### Monitoring Stack
- **Prometheus**: Metrics collection
- **Grafana**: Dashboard visualization
- **ELK Stack**: Log aggregation
- **AlertManager**: Alert notification

## 📈 Scaling Considerations

### Horizontal Scaling
- **Backend API**: Stateless, can scale horizontally
- **Bot Instances**: Can run multiple instances with different symbols
- **Database**: Read replicas for reporting
- **Cache**: Redis for session and data caching

### Vertical Scaling
- **PostgreSQL**: Increase memory and CPU
- **RabbitMQ**: More memory for message queues
- **Bot**: More CPU for complex calculations

## 🔒 Security Architecture

### Network Security
```
┌─────────────┐    HTTPS    ┌─────────────┐    Internal    ┌─────────────┐
│   Internet  │ ──────────► │   Reverse   │ ─────────────► │   Backend   │
│             │             │   Proxy     │    Network     │    API      │
└─────────────┘             └─────────────┘                └──────┬──────┘
                                                                  │
                                                                  │ Internal
                                                                  │ Network
                                                                  ▼
                                                           ┌─────────────┐
                                                           │   Database  │
                                                           │   & Bot     │
                                                           └─────────────┘
```

### Data Protection
- **Encryption at rest**: PostgreSQL TDE
- **Encryption in transit**: TLS 1.3
- **Secrets management**: HashiCorp Vault or AWS Secrets Manager
- **Audit logging**: Comprehensive audit trail

## 🐛 Troubleshooting Guide

### Common Issues and Solutions

| Issue | Symptoms | Solution |
|-------|----------|----------|
| Database connection failed | 500 errors, connection refused | Check PostgreSQL service, verify credentials |
| RabbitMQ connection issues | Messages not delivered | Check RabbitMQ health, verify network |
| Bot not detecting opportunities | No arbitrage alerts | Check exchange WebSocket connections |
| Dashboard not updating | Stale data, no real-time updates | Check SignalR connection, verify JWT token |
| Migration failures | Database schema errors | Check migration history, verify EF Core |

### Log Locations
- **Application logs**: Docker container stdout/stderr
- **Database logs**: PostgreSQL container logs
- **Message broker logs**: RabbitMQ container logs
- **Web server logs**: Nginx access/error logs

## 📚 Additional Resources

### Documentation
- [PostgreSQL Documentation](https://www.postgresql.org/docs/)
- [.NET 8 Documentation](https://learn.microsoft.com/en-us/dotnet/)
- [Rust Documentation](https://doc.rust-lang.org/)
- [RabbitMQ Documentation](https://www.rabbitmq.com/documentation.html)
- [Docker Documentation](https://docs.docker.com/)

### Monitoring Tools
- **pgAdmin**: PostgreSQL administration
- **RabbitMQ Management UI**: Message broker monitoring
- **Grafana**: Metrics visualization
- **Prometheus**: Time-series database

### Testing Tools
- **xUnit**: .NET unit testing
- **cargo test**: Rust testing framework
- **Jest**: React testing
- **Postman**: API testing

---

**Last Updated**: March 30, 2026  
**Maintainer**: System Architecture Team  
**Version**: 2.0.0 (PostgreSQL Migration)