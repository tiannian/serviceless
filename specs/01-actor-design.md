# Actor Design Principles

## Overview

The actor module implements a simple actor model in Rust, inspired by Actix. This document describes the core design principles and architecture of the actor system.

## Core Concepts

### Actor Model

The actor model is a mathematical model of concurrent computation that treats "actors" as the universal primitives of concurrent computation. In this implementation:

- Each actor is represented by a `Service`
- Actors communicate exclusively through asynchronous message passing
- Each actor processes messages sequentially in a single-threaded manner
- Actors are isolated from each other and do not share mutable state

### Key Components

#### Service Trait

The `Service` trait represents an actor that can process messages. Key characteristics:

- **Lifecycle Hooks**: Services can implement `started()` and `stopped()` hooks for initialization and cleanup
- **Async Processing**: All operations are asynchronous using async/await
- **Isolation**: Each service instance processes messages independently

#### Address

An `Address<S>` is a handle to a service that allows sending messages:

- **Cloneable**: Addresses can be cloned and shared across threads
- **Two Communication Modes**:
  - `call()`: Sends a message and waits for a result (request-response pattern)
  - `send()`: Sends a message without waiting for a result (fire-and-forget pattern)
- **Status Checking**: Can check if the service has stopped via `is_stop()`

#### Context

The `Context<S>` manages the message channel and service lifecycle:

- **Message Channel**: Contains an unbounded sender and receiver for messages
- **Address Generation**: Can create addresses before the service starts
- **Lifecycle Control**: Provides methods to pause and stop the service
- **Message Loop**: Manages the message processing loop when running

#### Handler Trait

The `Handler<M>` trait defines how a service processes a specific message type:

- **Type-Safe**: Each message type has its own handler implementation
- **Context Access**: Handlers receive mutable access to both the service and context
- **Result Type**: Each message type defines its own result type

#### Message Trait

The `Message` trait marks types that can be sent to actors:

- **Associated Result Type**: Each message type specifies its result type
- **Type Safety**: Ensures compile-time type checking for message handling

## Design Principles

### 1. Message Passing

All communication between actors happens through asynchronous message passing:

- Messages are sent via `Address` using `call()` or `send()`
- Messages are wrapped in `Envelope` structures for type erasure
- Results are communicated back through oneshot channels (for `call()`)

### 2. Type Safety

The design leverages Rust's type system for safety:

- Each message type is statically checked at compile time
- Handlers are type-safe and cannot be called with wrong message types
- The trait system ensures correct message handling implementations

### 3. Async/Await Integration

The actor system is built on async/await:

- All message handling is asynchronous
- Services run as async tasks that can be spawned on any async runtime
- The `run()` method returns a future that must be spawned by the caller

### 4. Envelope Pattern

Messages are wrapped in `Envelope` structures for type erasure:

- Allows storing different message types in the same channel
- Uses trait objects (`EnvelopProxy`) for dynamic dispatch
- Encapsulates both the message and optional result channel

### 5. Lifecycle Management

Services have explicit lifecycle hooks:

- `started()`: Called when the service begins processing messages
- Message processing loop: Continuously processes messages from the channel
- `stopped()`: Called when the service stops (channel closes)

### 6. Error Handling

The system defines specific error types:

- `ServiceStoped`: Returned when trying to send to a stopped service
- `ServicePaused`: Returned when a service is paused (currently not fully implemented)

### 7. No-Std Support

The actor system must support `no_std` environments:

- **Core Requirement**: The actor module should compile and function without the Rust standard library
- **Alloc Dependency**: Use the `alloc` crate for heap-allocated types (Vec, String, Box, etc.)
- **Feature Gating**: Standard library features should be optional and gated behind a `std` feature flag
- **Runtime Agnostic**: The actor system should remain runtime-agnostic and work with any async runtime that supports no_std
- **Core Types**: Use `core::future::Future` instead of `std::future::Future` where possible
- **Conditional Compilation**: Use feature gates to conditionally enable std-specific functionality

## Architecture Flow

### Service Startup

1. Create a `Service` instance
2. Call `start()` or `start_by_context()` to get an `Address` and future
3. Spawn the returned future on an async runtime
4. The service calls `started()` hook
5. The service enters the message processing loop

### Message Processing

1. Client obtains an `Address` to the service
2. Client calls `call()` or `send()` with a message
3. Message is wrapped in an `Envelope` and sent to the service's channel
4. Service receives the envelope from its receiver
5. Envelope dispatches to the appropriate handler based on message type
6. Handler processes the message and optionally sends result back

### Service Shutdown

1. `Context::stop()` is called, closing the channel
2. Message loop exits when receiver detects channel closure
3. Service calls `stopped()` hook
4. Service future completes

## Examples

### Basic Service

```rust
#[derive(Default)]
struct MyService;

#[async_trait]
impl Service for MyService {
    async fn started(&mut self, _ctx: &mut Context<Self>) {
        println!("Service started");
    }
}

struct MyMessage(String);

impl Message for MyMessage {
    type Result = String;
}

#[async_trait]
impl Handler<MyMessage> for MyService {
    async fn handle(&mut self, msg: MyMessage, _ctx: &mut Context<Self>) -> String {
        format!("Received: {}", msg.0)
    }
}
```

### Using a Service

```rust
let service = MyService::default();
let (addr, future) = service.start();
tokio::spawn(future);

// Request-response
let result = addr.call(MyMessage("Hello".to_string())).await?;

// Fire-and-forget
addr.send(MyMessage("Hello".to_string()))?;
```

## Design Decisions

### Unbounded Channels

The implementation uses unbounded channels (`UnboundedSender`/`UnboundedReceiver`):

- **Pros**: No backpressure, simple implementation, messages never block
- **Cons**: Potential for unbounded memory growth if producer is faster than consumer

### Type Erasure with Envelopes

Messages are type-erased using trait objects:

- **Pros**: Single channel can handle multiple message types
- **Cons**: Slight runtime overhead from dynamic dispatch

### Caller-Spawned Futures

Services return futures that must be spawned by the caller:

- **Pros**: Runtime-agnostic, caller controls execution context
- **Cons**: Requires explicit spawning, potential for misuse

### Cloneable Addresses

Addresses can be cloned and shared:

- **Pros**: Easy to share service handles across threads
- **Cons**: No built-in reference counting or ownership tracking

## Future Considerations

- Pause functionality is currently marked as unusable and may need refinement
- Error handling could be extended with more specific error types
- Consider adding bounded channels as an option for backpressure
- Potential for supervisor patterns and actor hierarchies
