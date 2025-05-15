# Lazarus

A powerful command-line tool for managing training programs and tracking your workout progress.

## Features

- Create and manage workout programs with custom exercises
- Track sets, reps, and weights during training sessions
- Automatically track personal records (PRs)
- Export and import your training database
- Swap exercises mid-session
- Add workout notes

## Building on Android (Termux)

To build Lazarus on an Android device using Termux, follow these steps:

1. Install required dependencies:
   ```
   pkg install clang make pkg-config fontconfig freetype libpng
   ```

2. Install Rust:
   ```
   pkg install rust
   ```

3. Clone the repository:
   ```
   git clone https://github.com/misterclayt0n/lazarus
   cd lazarus
   ```

4. Build and install the application:
   ```
   cargo build --release
   cp target/release/lazarus $PREFIX/bin/
   ```

## Database Migration (for c0utin)

If you're migrating from an older version of the database, you can use the database migration command:

```
lazarus db migrate <old_db>
```

This will:
1. Backup your existing database
2. Add the new `personal_records` table for PR tracking
3. Transfer existing PR data from your exercise history
4. Update the database schema to the latest version
5. Create a new `lazarus.db` database, with the new schema

**OBS**: If you're importing an existing database TOML file, the import will automatically calculate PRs from your exercise history.

## Commands Reference
Lazarus works with indeces as much as it can, so whenever you see something like: `<program_name> || <program_id>`, it means this command accepts either a string of the program name (e.g. "Program 1"), or it's global index (e.g. 1).

### Programs and Blocks
- `program list` - List all training programs.
- `program show <program_name> || <program_id>` - Show a single program in detail.
- `program delete <program_name> || <program_id>` - Delete a program.
- `program import <files...>` - Import one or more programs.

### Exercises
- `exercise add <name> --muscle <muscle> [--desc <description>]` - Add a new exercise.
- `exercise list [--muscle <muscle>]` - List all exercises.
- `exercise show [--graph] <exercise_name> || <exercise_id>` - Show detailed exercise information (use `--graph` to show a progression graph).
- `exercise delete <exercise_name> || <exercise_id>` - Delete an exercise.
- `exercise import <file>` - Import exercises from a TOML file.

### Sessions
- `session start <program_name> || <program_id> <block_name> || <block_id>` - Start a new training session.
- `session show` - Show the current active session.
- `session edit <exercise_id> <weight> <reps> [--set <set>] [--new]` - Log a set for an exercise. The session order is inferred, use `--set` to edit a particular set, and use `--new` with you want to edit a new set. 
**OBS**: `<exercise_id>` here means the id of the session, not the global exercise index shown in `exercise list`.  
- `session swap <exercise_id> <new_exercise_name> || <new_exercise_id>` - Swap an exercise with a different one.
**OBS**: `<exercise_id>` here means the id of the session, not the global exercise index shown in `exercise list`.
- `session add-ex <exercise_name> || <exercise_id> <sets>` - Add a new exercise to the current session with a given amount of sets.
- `session note <exercise> <note>` - Add a note to an exercise.
- `session end` - End the current training session.
- `session log --date <date>` - View a completed session by date (format: DD-MM-YYYY)
- `session cancel` - Cancel the current session.

### Database Management
- `db export [--file <file>]` - Export the database to a TOML file.
- `db import <file>` - Import from a TOML file.
- `db migrate <old_db>` - Migrate an old lazaro.db into the current one.

### Configuration
- `config list` - Show all config keys
- `config get <key>` - Get the value of a key
- `config set <key> <val>` - Set or override a key
- `config unset <key>` - Remove a key

### Calendar
- `calendar [--year <year>] [--month <month>]` - Show training sessions in a calendar view

## License

This project is licensed under the MIT License. 