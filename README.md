# EvolveMe Backend

This is the backend service for the EvolveMe health application. It provides a simple API for syncing health data from the iOS client app.

## Features

- User authentication (register/login) with JWT tokens
- Health data synchronization
- PostgreSQL database for data storage

## Requirements

- Rust 1.70 or higher
- PostgreSQL 12 or higher
- Docker and Docker Compose (optional, for containerized setup)

## Getting Started

### Environment Setup

1. Clone the repository:

```bash
git clone https://github.com/yourusername/evolveme-backend.git
cd evolveme-backend
```

2. Create a `.env` file based on the example:

```bash
cp .env.example .env
```

3. Edit the `.env` file to set appropriate values for your environment.

### Database Setup

Ensure PostgreSQL is running and create a database:

```bash
psql -U postgres
CREATE DATABASE evolveme;
```

### Running the Application

#### Without Docker

1. Install dependencies and run the application:

```bash
cargo run
```

2. The server will start on http://127.0.0.1:8080 (or the HOST/PORT specified in your .env file).

#### With Docker

1. Build and start the containers:

```bash
docker-compose up -d
```

2. The server will be available on http://localhost:8080.

## API Endpoints

### Authentication

- `POST /auth/register` - Register a new user
  - Request body: `{ "email": "user@example.com", "username": "username", "password": "password" }`

- `POST /auth/login` - Login and get JWT token
  - Request body: `{ "email": "user@example.com", "password": "password" }`

### Health Data

- `POST /health-data` - Sync health data from client
  - Requires Authorization header with Bearer token
  - Request body:
  ```json
  {
    "device_id": "device-unique-id",
    "timestamp": "2025-05-08T12:00:00Z",
    "steps": 8500,
    "heart_rate": 75.5,
    "sleep": {
      "total_sleep_hours": 7.5,
      "in_bed_time": 1715000000,
      "out_bed_time": 1715027000,
      "time_in_bed": 8.2
    },
    "active_energy_burned": 350.5,
    "additional_metrics": {
      "custom_data": "value"
    }
  }
  ```

## Development

### Running Tests

```bash
cargo test
```

### Database Migrations

Migrations are automatically applied when the application starts. To create new migrations:

```bash
cargo install sqlx-cli
sqlx migrate add migration_name
```

## License

[MIT](LICENSE)