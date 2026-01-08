# Architecture Documentation

[Documentation](../README.md) > Architecture

---

## Overview

This section documents SpiritStream's software architecture, from high-level system context through detailed component relationships and security considerations.

## Documents

| Document | Description | Audience |
|----------|-------------|----------|
| [01. System Overview](./01-system-overview.md) | High-level architecture with C4 diagrams | All levels |
| [02. Component Architecture](./02-component-architecture.md) | Detailed component breakdown | Intermediate+ |
| [03. Data Flow](./03-data-flow.md) | Data flow and sequence diagrams | Intermediate+ |
| [04. Security Architecture](./04-security-architecture.md) | Security model, encryption, permissions | Advanced |

## Key Concepts

- **Three-Tier Architecture**: Presentation (React), Application (Rust), Infrastructure (FFmpeg)
- **Tauri IPC**: Type-safe communication between frontend and backend
- **Layered Security**: Capabilities, encryption, path validation

## Quick Links

- [Technology Stack](./01-system-overview.md#iv-technology-stack)
- [Container Diagram](./01-system-overview.md#iii-container-architecture)
- [Security Model](./04-security-architecture.md)

---

*Section: 01-architecture*
