Eloquentle

A command-line Wordle solver and assistant built in Rust. Eloquentle helps you solve Wordle puzzles efficiently through advanced filtering and word recommendation algorithms.

## Features

- **Interactive TUI**: Clean, responsive terminal user interface built with Ratatui
- **Smart Word Suggestions**: Get optimal guesses based on information theory principles
- **Game Simulation**: Test strategies against the built-in Wordle game engine
- **Detailed Feedback**: See why words are recommended and how they narrow down possibilities
- **Word Bank**: Includes comprehensive dictionary of valid Wordle words

## Installation

### Prerequisites
- Rust and Cargo (latest stable version recommended)

### Building from Source
```
git clone https://github.com/yourusername/eloquentle.git
cd eloquentle
cargo build --release
```

The built executable will be located at `target/release/eloquentle`.

## Usage

### Starting the Solver
```
./target/release/eloquentle
```

### How to Use
1. Start with the recommended first word guess
2. Enter the feedback from the actual Wordle game:
   - Green letter (correct position): Enter the letter in uppercase
   - Yellow letter (wrong position): Enter the letter in lowercase
   - Gray letter (not in word): Enter a dot (.)
3. Get the next best guess recommendation
4. Repeat until you solve the puzzle

## Project Structure

- `src/` - Core library functionality
  - `filter.rs` - Word filtering based on feedback
  - `game.rs` - Wordle game logic
  - `info.rs` - Information representation
  - `ranking.rs` - Word ranking strategies
  - `words.rs` - Word list management
- `bin/` - Terminal UI application
- `word-bank.csv` - Complete dictionary of valid words
- `valid-words.csv` - Possible solution words

## Development

### Running Tests
```
cargo test
```

### Building Documentation
```
cargo doc --open
```

## How It Works

Eloquentle uses information theory to calculate which guesses will eliminate the most possibilities. After each guess, it filters the remaining word list based on the feedback, then recommends the most efficient next guess.

The core algorithm evaluates each candidate word based on how well it would partition the remaining solution space, regardless of whether the word itself is a likely solution.
