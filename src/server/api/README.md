# API Module Refactoring

This directory contains the refactored API implementation that was previously all contained in the
main `server/mod.rs` file. The refactoring separates the API logic into individual modules based on
model types, making the codebase more maintainable and organized.

## Structure

The API module is organized as follows:

```
api/
├── mod.rs                    # Common utilities and re-exports
├── events.rs                 # Event-related API endpoints
├── workflows.rs              # Workflow-related API endpoints
├── jobs.rs                   # Job-related API endpoints
├── files.rs                  # File-related API endpoints
├── results.rs                # Result-related API endpoints
├── compute_nodes.rs          # Compute node-related API endpoints
├── schedulers.rs             # Scheduler-related API endpoints
├── resource_requirements.rs  # Resource requirements-related API endpoints
└── user_data.rs              # User data-related API endpoints
```

## Common Components

### `ApiContext`

A shared context object that contains the database connection pool and other shared resources:

```rust
#[derive(Clone)]
pub struct ApiContext {
    pub pool: Arc<SqlitePool>,
}
```

### Common Utilities

- `database_error()` - Standardized database error handling
- `json_parse_error()` - JSON parsing error handling
- `PaginationInfo` - Common pagination response structure
- `MAX_RECORD_TRANSFER_COUNT` - Shared constant for maximum record limits

## Module Structure

Each API module follows a consistent pattern:

1. **Trait Definition** - Defines the API interface (e.g., `EventsApi<C>`)
2. **Implementation Struct** - Contains the context (e.g., `EventsApiImpl`)
3. **Implementation Block** - Implements the trait for the struct

Example structure:

```rust
#[async_trait]
pub trait EventsApi<C> {
    async fn create_event(&self, body: models::EventModel, context: &C) -> Result<CreateEventResponse, ApiError>;
    // ... other methods
}

#[derive(Clone)]
pub struct EventsApiImpl {
    pub context: ApiContext,
}

#[async_trait]
impl<C> EventsApi<C> for EventsApiImpl
where C: Has<XSpanIdString> + Send + Sync
{
    async fn create_event(&self, body: models::EventModel, context: &C) -> Result<CreateEventResponse, ApiError> {
        // Implementation
    }
}
```

## Integration with Main Server

The main `Server` struct now contains instances of each API implementation:

```rust
pub struct Server<C> {
    marker: PhantomData<C>,
    pool: Arc<SqlitePool>,
    events_api: EventsApiImpl,
    workflows_api: WorkflowsApiImpl,
    jobs_api: JobsApiImpl,
    // ... other API implementations
}
```

The main API trait implementation delegates to the appropriate sub-module:

```rust
impl<C> Api<C> for Server<C> {
    async fn create_event(&self, body: models::EventModel, context: &C) -> Result<CreateEventResponse, ApiError> {
        use api::EventsApi;
        self.events_api.create_event(body, context).await
    }
}
```

## Benefits of This Refactoring

1. **Separation of Concerns** - Each module focuses on a specific domain model
2. **Improved Maintainability** - Smaller, focused files are easier to understand and modify
3. **Better Code Organization** - Related functionality is grouped together
4. **Easier Testing** - Individual modules can be tested in isolation
5. **Reduced Compilation Time** - Changes to one module don't require recompiling the entire server
6. **Team Development** - Multiple developers can work on different modules simultaneously

## Migration Status

### Fully Migrated

- ✅ Events API - All event-related endpoints moved to `events.rs`
- ✅ Workflows API - All workflow-related endpoints moved to `workflows.rs`
- ✅ Jobs API - Core job functionality moved to `jobs.rs`
- ✅ Files API - File operations moved to `files.rs`

### Partially Migrated (Stubs Created)

- 🚧 Results API - Structure in place, implementations needed
- 🚧 Compute Nodes API - Structure in place, implementations needed
- 🚧 Schedulers API - Structure in place, implementations needed
- 🚧 Resource Requirements API - Structure in place, implementations needed
- 🚧 User Data API - Structure in place, implementations needed

### Implementation Notes

1. **Database Operations** - All database operations use the shared `ApiContext.pool`
2. **Error Handling** - Consistent error handling using `database_error()` and `json_parse_error()`
3. **Logging** - Maintains existing logging patterns with `info!()` and `debug!()`
4. **Response Types** - All original response types are preserved

### Next Steps

1. Complete the implementation of remaining stub methods
2. Add comprehensive unit tests for each module
3. Consider adding integration tests
4. Update API documentation to reflect the new structure
5. Consider extracting common database operations into helper functions

## Usage Example

```rust
// Creating the server with API modules
let pool = SqlitePool::connect("database.db").await?;
let server = Server::new(pool);

// The server automatically delegates to appropriate modules
let response = server.create_event(event_model, &context).await?;
```

This refactoring maintains full backward compatibility while providing a much cleaner and more
maintainable codebase structure.
