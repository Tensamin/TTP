# Tensamin Transport Protocol

TTP is a transport-layer library.

## Project overview
- TTP defines lightweight transport primitives and reference behavior for message delivery, connection management, and optional congestion control features.
- The repository separates concerns:
  - protocol core logic (pure Rust, platform-agnostic)
  - native / platform-specific integration code (FFI, platform APIs, or optimizations)

## Repository structure
- `core/` core protocol implementation
- `native/` platform/native integrations
- `cli/` in progress CLI to en-/decode TTP messages

## Core
The core section consists of Communication Value encoding.
A Communication Value consists of:
- a Communication Type,
- an optional sender ID,
- an optional receiver ID,
- an optional Message ID,
- a Data Value of type Conatiner.

A DataValue may be:
- a `Container`,
  - consists of DataTypes mapped to DataValues
- an `Array`,
  - consists of DataValues
- a `Boolean`,
- a `Number` (rust i64),
- a `String`,
- `Null`.

A DataType is a 1 byte number, arbitrarily mapped to a defined name.
The list of standart DataTypes mapped to their binary value can be found at `core/src/data_types.rs`.

## Native
The Native crate creates basic communication via WTransport. It allows for communication between the Tensamin Processes.

It consists of the `Connection`, the `Host` and the `Client`.

The Connection is split into the `Sender` and `Receiver`.
Messages can be send from one Partys `Sender` to the other Partys `Receiver`.

The `Host` accepts `Client` connection requests.
The `Host`s `.next()` function repeatedly produces `Sender`s and `Receiver`s when `Client`s connect.

The `Client` produces a `Sender` and `Receiver` when the `.connect()` function is called.
