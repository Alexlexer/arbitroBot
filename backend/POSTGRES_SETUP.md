# PostgreSQL Backend Setup for ArbitHub

## Overview

This document describes the new PostgreSQL-based backend implementation for the ArbitHub dashboard. The system now uses Entity Framework Core with PostgreSQL for data persistence, replacing the previous SQLite-based authentication system.

## Architecture

### Database Schema

The PostgreSQL database includes the following main tables:

1. **Users** - User accounts with roles (Viewer, Trader, Admin, System)
2. **UserSessions** - Active user sessions for session management
3. **RefreshTokens** - JWT refresh tokens for extended authentication
4. **TradeHistories** - Historical trade records with P&L tracking
5. **ArbitrageOpportunities** - Detected arbitrage opportunities
6. **ExchangeBalances** - Exchange account balances and equity
7. **AuditLogs** - Security audit trail for compliance
8. **ApiKeys** - API key management for external integrations
9. **FailedLoginAttempts** - Security monitoring for brute force attacks
10. **SystemConfigs** - System configuration key-value store

### Technology Stack

- **.NET 8** - Backend framework
- **Entity Framework Core 8** - ORM for database access
- **PostgreSQL 16** - Primary database
- **Npgsql** - PostgreSQL provider for EF Core
- **JWT + BCrypt** - Authentication and password hashing
- **SignalR** - Real-time WebSocket communication
- **RabbitMQ** - Message broker for bot communication

## Setup Instructions

### 1. Local Development

#### Prerequisites
- .NET 8 SDK
- PostgreSQL 16+ (or Docker)
- RabbitMQ (or Docker)

#### Option A: Using Docker Compose (Recommended)

```bash
# Start all services
docker-compose up -d

# Check services
docker-compose ps

# View logs
docker-compose logs -f arbitro-backend
```

#### Option B: Manual Setup

1. **Install PostgreSQL**
   ```bash
   # Ubuntu/Debian
   sudo apt install postgresql postgresql-contrib
   
   # macOS
   brew install postgresql
   
   # Windows
   # Download from https://www.postgresql.org/download/windows/
   ```

2. **Create Database**
   ```sql
   CREATE DATABASE arbit_hub;
   CREATE USER arbit_user WITH PASSWORD 'your_password';
   GRANT ALL PRIVILEGES ON DATABASE arbit_hub TO arbit_user;
   ```

3. **Update Configuration**
   ```json
   // backend/appsettings.json
   {
     "ConnectionStrings": {
       "DefaultConnection": "Host=localhost;Database=arbit_hub;Username=arbit_user;Password=your_password;Port=5432"
     }
   }
   ```

4. **Run Migrations**
   ```bash
   cd backend
   dotnet ef database update
   ```

5. **Run Backend**
   ```bash
   cd backend
   dotnet run
   ```

### 2. Production Deployment

#### Environment Variables

Set the following environment variables in production:

```bash
# Database
ConnectionStrings__DefaultConnection=Host=postgres-prod;Database=arbit_hub_prod;Username=arbit_user;Password=strong_password;Port=5432

# JWT (64+ character random string)
Jwt__Key=your_64_char_random_secret_key_here

# Security
Auth__InviteCode=your_invite_code_here

# RabbitMQ
RabbitMq__Host=rabbitmq-prod
RabbitMq__Port=5672
RabbitMq__Username=arbit_user
RabbitMq__Password=rabbitmq_password
```

#### Docker Compose for Production

Create `docker-compose.prod.yml`:

```yaml
version: '3.8'

services:
  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: ${POSTGRES_DB}
      POSTGRES_USER: ${POSTGRES_USER}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
    restart: unless-stopped
    networks:
      - arbitro-network

  rabbitmq:
    image: rabbitmq:3-management-alpine
    environment:
      RABBITMQ_DEFAULT_USER: ${RABBITMQ_USER}
      RABBITMQ_DEFAULT_PASS: ${RABBITMQ_PASSWORD}
    volumes:
      - rabbitmq_data:/var/lib/rabbitmq
    restart: unless-stopped
    networks:
      - arbitro-network

  arbitro-backend:
    build:
      context: ./backend
      dockerfile: Dockerfile
    environment:
      - ConnectionStrings__DefaultConnection=Host=postgres;Database=${POSTGRES_DB};Username=${POSTGRES_USER};Password=${POSTGRES_PASSWORD};Port=5432
      - Jwt__Key=${JWT_KEY}
      - Auth__InviteCode=${INVITE_CODE}
      - RabbitMq__Host=rabbitmq
      - RabbitMq__Port=5672
      - RabbitMq__Username=${RABBITMQ_USER}
      - RabbitMq__Password=${RABBITMQ_PASSWORD}
    depends_on:
      - postgres
      - rabbitmq
    restart: unless-stopped
    networks:
      - arbitro-network

volumes:
  postgres_data:
  rabbitmq_data:

networks:
  arbitro-network:
    driver: bridge
```

## API Endpoints

### Authentication
- `POST /api/auth/login` - User login
- `POST /api/auth/register` - User registration (requires invite code)

### Database Testing
- `GET /api/test/database` - Test database connection
- `GET /api/test/migrations` - Check migration status
- `POST /api/test/migrate` - Apply migrations (Admin only)

### History
- `GET /api/history?days=30` - Get trade history (JWT required)

### SignalR Hub
- `GET /hubs/arbit` - Real-time WebSocket connection

## Security Features

### 1. Password Security
- BCrypt hashing with work factor 12
- Minimum 8 character passwords
- Password strength validation

### 2. JWT Authentication
- 24-hour token expiration (configurable)
- Role-based authorization
- Token validation with signing key

### 3. Audit Logging
- All authentication events logged
- Failed login attempt tracking
- Brute force attack detection
- 90-day retention policy

### 4. Session Management
- 7-day session expiration
- IP address and user agent tracking
- Manual session revocation

### 5. Rate Limiting
- Failed login attempt limits
- IP-based brute force protection
- Configurable thresholds

## Migration from SQLite

### Data Migration Script

```sql
-- Export from SQLite
sqlite3 data/users.db .dump > users_backup.sql

-- Convert and import to PostgreSQL
-- Note: Manual data migration required due to schema differences
-- Focus on user accounts and critical data
```

### Configuration Changes

1. Update `appsettings.json` with PostgreSQL connection string
2. Remove SQLite-specific configuration
3. Update Docker Compose to include PostgreSQL service
4. Update environment variables for production

## Monitoring and Maintenance

### Database Maintenance

```bash
# Regular backups
pg_dump -U postgres arbit_hub > backup_$(date +%Y%m%d).sql

# Vacuum and analyze
psql -U postgres -d arbit_hub -c "VACUUM ANALYZE;"

# Check table sizes
psql -U postgres -d arbit_hub -c "
SELECT schemaname, tablename, pg_size_pretty(pg_total_relation_size(schemaname||'.'||tablename)) 
FROM pg_tables 
WHERE schemaname NOT IN ('pg_catalog', 'information_schema') 
ORDER BY pg_total_relation_size(schemaname||'.'||tablename) DESC;
"
```

### Health Checks

```bash
# Database connection
curl http://localhost:5000/api/test/database

# Service health
curl http://localhost:5000/health

# RabbitMQ connection
# Check via management UI: http://localhost:15672
```

### Logging

Logs are available via:
- Docker Compose: `docker-compose logs -f arbitro-backend`
- Application logs: `backend/logs/` directory
- Audit logs: PostgreSQL `audit_logs` table

## Troubleshooting

### Common Issues

1. **Database Connection Failed**
   ```
   Error: Connection refused
   Solution: Check PostgreSQL is running and credentials are correct
   ```

2. **Migration Errors**
   ```
   Error: Relation already exists
   Solution: Drop database and recreate: dropdb arbit_hub && createdb arbit_hub
   ```

3. **JWT Validation Failed**
   ```
   Error: Invalid token
   Solution: Ensure JWT key is set and consistent across services
   ```

4. **RabbitMQ Connection Issues**
   ```
   Error: Connection to RabbitMQ failed
   Solution: Check RabbitMQ service and firewall settings
   ```

### Debug Mode

Enable detailed logging in `appsettings.Development.json`:

```json
{
  "Logging": {
    "LogLevel": {
      "Default": "Debug",
      "Microsoft.EntityFrameworkCore.Database.Command": "Information"
    }
  }
}
```

## Performance Considerations

### Database Indexing
- Automatic indexes on foreign keys
- Custom indexes on frequently queried columns
- Regular index maintenance with `REINDEX`

### Connection Pooling
- Default EF Core connection pooling (100 connections)
- Configure via connection string: `Pooling=true;MaxPoolSize=200`

### Caching Strategy
- Consider Redis for session caching
- Query result caching for frequently accessed data
- CDN for static assets

## Scaling

### Vertical Scaling
- Increase PostgreSQL memory (`shared_buffers`)
- Add more CPU cores
- Use SSD storage

### Horizontal Scaling
- Read replicas for reporting
- Connection pooling with PgBouncer
- Load balancing for backend instances

### High Availability
- PostgreSQL replication with streaming
- RabbitMQ clustering
- Load balancer with health checks

## Next Steps

### Phase 2: Enhanced Features
1. User management UI (Admin panel)
2. Advanced analytics and reporting
3. Email notifications and alerts
4. Two-factor authentication

### Phase 3: Advanced Security
1. IP whitelisting/blacklisting
2. Advanced rate limiting
3. Security headers and CSP
4. Regular security audits

### Phase 4: Monitoring
1. Prometheus metrics
2. Grafana dashboards
3. Alert manager integration
4. Performance monitoring

## Support

For issues and questions:
1. Check logs in `backend/logs/`
2. Review audit logs in database
3. Test database connection with `/api/test/database`
4. Check migration status with `/api/test/migrations`

## License

Proprietary - See LICENSE file for details.