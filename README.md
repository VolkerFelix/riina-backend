# EvolveMe Backend

EvolveMe Backend is a RESTful API service that handles health data uploads from mobile and wearable devices. The service provides authentication, data storage, and data retrieval endpoints.

## Features

- User authentication with JWT
- Secure data storage in PostgreSQL database
- Health data upload endpoints for various metrics:
  - Heart rate data
  - Blood oxygen levels
  - Sleep data
- Containerized deployment with Docker
- CI/CD pipeline with GitHub Actions

## Technology Stack

- **Language**: Rust
- **Web Framework**: Actix Web 4
- **Database**: PostgreSQL with SQLx
- **Authentication**: JWT (JSON Web Tokens)
- **Logging**: Tracing with Bunyan formatter
- **Configuration**: Environment variables + YAML files
- **Containerization**: Docker
- **CI/CD**: GitHub Actions

## Project Structure

```
evolveme-backend/
├── .github/            # GitHub Actions workflows
├── .sqlx/              # SQLx prepared queries
├── configuration/      # Configuration files
├── migrations/         # Database migrations
├── scripts/            # Utility scripts
├── src/                # Source code
│   ├── config/         # Configuration handling
│   ├── handlers/       # Request handlers
│   ├── middleware/     # Middleware components
│   ├── models/         # Data models
│   ├── routes/         # API routes
│   ├── utils/          # Utility functions
│   ├── lib.rs          # Library entry point
│   ├── main.rs         # Application entry point
│   └── telemetry.rs    # Telemetry setup
└── tests/              # Integration tests
```

## Getting Started

### Prerequisites

- Rust 1.74 or higher
- PostgreSQL 14 or higher
- Docker (optional, for containerized development)

### Local Development Setup

1. **Clone the repository**

```bash
git clone https://github.com/yourusername/evolvemebackend.git
cd evolvemebackend
```

2. **Setup environment variables**

Create a `.env` file in the project root with the following variables:

```
POSTGRES__DATABASE__USER=postgres
POSTGRES__DATABASE__PASSWORD=password
APP__APPLICATION__USER=app_user
APP__APPLICATION__PASSWORD=app_password
JWT_SECRET=your_jwt_secret_key_here
APP_ENVIRONMENT=local
```

3. **Setup the database**

```bash
# Run the database setup script
./scripts/init_db_and_redis.sh
```

4. **Build and run the application**

```bash
cargo build
cargo run
```

The server will start at http://localhost:8080

### Docker Setup

1. **Build the Docker image**

```bash
docker build -t evolveme-backend .
```

2. **Run with Docker Compose**

```bash
docker-compose up
```

## API Documentation

For detailed API documentation, see [API.md](API.md).

### Quick API Overview

- **Authentication**
  - `POST /register_user` - Register a new user
  - `POST /login` - Login and get JWT token

- **Health Data Endpoints** (require authentication)
  - `POST /health/upload_health` - Upload health data (includes all health metrics)
  - `GET /protected/resource` - Access protected resources

- **System Health**
  - `GET /backend_health` - Check service health

## Running Tests

```bash
# Run all tests
cargo test

# Run specific test
cargo test backend_health_working

# Run with logs visible
TEST_LOG=true cargo test
```

## Contributing

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Run tests and ensure they pass (`cargo test`)
4. Commit your changes (`git commit -m 'Add some amazing feature'`)
5. Push to the branch (`git push origin feature/amazing-feature`)
6. Open a Pull Request

## License

This project is licensed under the Apache License 2.0 - see the [LICENSE](LICENSE) file for details.