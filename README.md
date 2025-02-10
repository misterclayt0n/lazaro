# Lazaro

**Lazaro** is a CLI training app inspired by Boostcamp that lets you manage your workouts with ease. With Lazaro, you can create and manage training sessions, exercises, and training programs—all from the command line. The goal is to keep things as simple as possible: just run `lazaro init` to create a local `lazaro.db` file in the current directory, and then use the available commands to manage your workouts.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Makefile](#makefile)
- [Usage](#usage)
  - [Database Initialization](#database-initialization)
  - [Training Sessions](#training-sessions)
  - [Exercises](#exercises)
  - [Training Programs](#training-programs)
  - [Exporting and Importing Data](#exporting-and-importing-data)
  - [Other Commands](#other-commands)
- [Examples](#examples)
- [Contributing](#contributing)
- [License](#license)
- [Contact](#contact)

## Features

- **Simple Initialization:** Run `lazaro init` to create and initialize the SQLite database (`lazaro.db`) in the current directory.
- **Training Sessions:** Start, end, cancel, and view detailed information about your training sessions.
- **Exercise Management:** Add new exercises or import them from a TOML file.
- **Training Programs:** Create, list, update, and delete training programs from TOML files.
- **Visualizations:** View a calendar of your training sessions, check your training history, and see overall statistics.
- **Data Export/Import:** Easily export your database data to a TOML file and rebuild your database from such a file.

## Installation

### Prerequisites

- [Go](https://golang.org/dl/) 1.16 or higher

### Clone the Repository

```bash
git clone https://github.com/misterclayt0n/lazaro
cd lazaro
```

To build Lazaro, run:

```bash
make build
```

To clean up build artifacts:

```bash
make clean
```

## Usage

Lazaro is designed to be simple and straightforward. Below is an overview of the available commands.

### Database Initialization

Initialize the database in the current directory by running:

```bash
./lazaro init
```

This command creates a `lazaro.db` file and sets up the database schema.

### Training Sessions

- **Start a session:**

  ```bash
  ./lazaro start-session --program "MyProgram" --block "Workout A" [--week 1]
  ```

  Starts a new training session for the specified program and block (and week, if applicable).

- **End the current session:**

  ```bash
  ./lazaro end-session
  ```

- **Cancel the current session:**

  ```bash
  ./lazaro cancel-session
  ```

- **View current session details:**

  ```bash
  ./lazaro show-session
  ```

- **Look up a session by its ID or date:**

  ```bash
  ./lazaro look-session [session-id]
  ./lazaro look-session --date 07/02/25
  ```

### Exercises

- **Add a new exercise:**

  ```bash
  ./lazaro add-exercise --name "Bench Press" --muscle "Chest" --description "Flat bench press with barbell"
  ```

- **Import exercises from a TOML file:**

  ```bash
  ./lazaro import-exercises exercises.toml
  ```

- **View exercise details and history:**

  ```bash
  ./lazaro show-ex "Bench Press"
  ```

### Training Programs

- **Create a new training program from a TOML file:**

  ```bash
  ./lazaro create-program program.toml
  ```

- **List all programs:**

  ```bash
  ./lazaro list-programs
  ```

- **Update an existing program without losing session data:**

  ```bash
  ./lazaro update-program program.toml
  ```

- **Delete a program:**

  ```bash
  ./lazaro delete-program program.toml
  ```

- **Display a complete program (optionally filtering by day):**

  ```bash
  ./lazaro show-program "MyProgram" --day "Workout A"
  ```

### Exporting and Importing Data

- **Export the database to a TOML file:**

  ```bash
  ./lazaro export mydump.toml
  ```

  If no file name is provided, a default file (e.g., `db_dump.toml`) is used in the default directory.

- **Rebuild the database from a TOML dump:**

  ```bash
  ./lazaro build-db mydump.toml
  ```

### Other Commands

- **View the training calendar:**

  ```bash
  ./lazaro calendar [month] [year]
  ```

- **Display overall statistics (total weight lifted, sessions, gym hours, etc.):**

  ```bash
  ./lazaro status
  ```

- **Swap an exercise variation during a session:**

  ```bash
  ./lazaro swap-ex 2 "VariationX"
  ```

## Examples

### Example 1: Starting a Session

```bash
./lazaro start-session --program "Hypertrophy2025" --block "Day A" --week 1
```

Starts a session for the program "Hypertrophy2025" on "Day A" for week 1.

### Example 2: Exporting the Database

```bash
./lazaro export workout_dump.toml
```

Exports the current database to `workout_dump.toml`.

### Example 3: Displaying the Calendar

```bash
./lazaro calendar 2 2025
```

Displays the calendar for February 2025, marking the days with training sessions.

## Contributing
Just make a branch, make your thing and PR.

## License

Distributed under the MIT License. See the [LICENSE](LICENSE) file for details.
