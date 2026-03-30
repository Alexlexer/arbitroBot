-- PostgreSQL initialization script for ArbitHub
-- This script runs when the PostgreSQL container is first created

-- Create extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";

-- Create read-only user for monitoring (optional)
-- CREATE USER arbitro_monitor WITH PASSWORD 'monitor_password';
-- GRANT CONNECT ON DATABASE arbit_hub TO arbitro_monitor;
-- GRANT USAGE ON SCHEMA public TO arbitro_monitor;
-- GRANT SELECT ON ALL TABLES IN SCHEMA public TO arbitro_monitor;
-- ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT ON TABLES TO arbitro_monitor;

-- Set search path
SET search_path TO public;

-- Note: Tables will be created by Entity Framework Core migrations
-- This script is for any additional database configuration needed