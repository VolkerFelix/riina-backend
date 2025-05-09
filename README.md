# Areum Health Backend

Areum Health Backend is a RESTful API service that handles health data uploads from mobile and wearable devices. The service provides authentication, data storage, and data retrieval endpoints for various health metrics including acceleration, heart rate, blood oxygen, skin temperature, and GPS location.

## Features

- User authentication with JWT
- Secure data storage in PostgreSQL database
- Health data upload endpoints for various metrics:
  - Acceleration data
  - Heart rate data
  - Blood oxygen levels
  - Skin temperature
  - GPS location
- Combined data queries (e.g., health data with corresponding GPS locations)
- Containerized deployment with Docker
- CI/CD pipeline with GitHub Actions
- Deployment to Fly.io

## Technology Stack

- **Language**: Rust
- **Web Framework**: Actix Web 4
- **Database**: PostgreSQL with SQLx
- **Authentication**: JWT (JSON Web Tokens)
- **Logging**: Tracing with Bunyan formatter
- **Configuration**: Environment variables + YAML files
- **Containerization**: Docker
- **CI/CD**: GitHub Actions
- **Hosting**: Fly.io

## Project Structure

```
areum-backend/
├── .github/            # GitHub Actions workflows
├── .sqlx/              # SQLx prepared queries
├── configuration/      # Configuration files
├── health_data/        # Example health data files
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
git clone https://github.com/yourusername/backendareum.git
cd backendareum
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
./scripts/init_db.sh
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
docker build -t areum-backend .
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
  - `POST /health/upload_acceleration` - Upload acceleration data
  - `GET /health/acceleration_data` - Get user's acceleration data
  - `POST /health/upload_heart_rate` - Upload heart rate data
  - `GET /health/heart_rate_data` - Get user's heart rate data
  - `POST /health/upload_blood_oxygen` - Upload blood oxygen data
  - `GET /health/blood_oxygen_data` - Get user's blood oxygen data
  - `POST /health/upload_skin_temperature` - Upload skin temperature data
  - `GET /health/skin_temperature_data` - Get user's skin temperature data
  - `POST /health/upload_gps_location` - Upload GPS location data
  - `GET /health/gps_location_data` - Get user's GPS location data
  - `GET /health/health_data_with_gps` - Get health data with corresponding GPS data

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

## Deployment

The application is set up for continuous deployment to Fly.io:

1. Commits to the main branch trigger the CI/CD pipeline
2. Tests are run
3. If tests pass, a Docker image is built and pushed
4. The application is deployed to Fly.io

Manual deployment can be performed with:

```bash
flyctl deploy
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