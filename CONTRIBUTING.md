# Contributing to Audoxidy

Thank you for your interest in contributing to **Audoxidy**! We welcome contributions from everyone.

## Introduction

Audoxidy is a modern, high-fidelity audio player written in Rust. Our goal is to create the best audio experience on Linux, Windows, and macOS.

## Initial Setup

1.  **Fork the repository** on GitHub.
2.  **Clone your fork**:
    ```bash
    git clone https://github.com/ZiggyCrue/Audoxidy.git
    cd Audoxidy
    ```
3.  **Install dependencies** (see README.md).
4.  **Verify build**:
    ```bash
    cargo check
    cargo test
    ```

## Submitting Changes

1.  Create a new branch for your feature or bugfix:
    ```bash
    git checkout -b feature/my-new-feature
    ```
2.  Make your changes and ensure they compile.
3.  Run `cargo fmt` to format your code.
4.  Commit your changes with descriptive messages.
5.  Push to your fork and submit a **Pull Request**.

## Code Style

- We follow standard Rust idioms.
- Please run `cargo fmt` and `cargo clippy` before submitting.

## License

By contributing, you agree that your contributions will be licensed under the execution of the GNU General Public License v3.0.
