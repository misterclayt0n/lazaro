package storage

import (
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"github.com/BurntSushi/toml"
)

// ExportDBToTOML exports all data from the database into a single TOML file.
// It queries sqlite_master for all user tables, then for each table,
// it retrieves all rows (as maps from column names to values) and writes
// the result to ~/.config/lazaro/db_dump.toml.
func ExportDBToTOML(outputPath string) error {
	st := NewStorage()

	tablesQuery := `SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%';`
	rows, err := st.DB.Query(tablesQuery)
	if err != nil {
		return fmt.Errorf("querying sqlite_master: %w", err)
	}
	defer rows.Close()

	dbDump := make(map[string][]map[string]interface{})

	for rows.Next() {
		var tableName string
		if err := rows.Scan(&tableName); err != nil {
			return fmt.Errorf("scanning table name: %w", err)
		}

		query := fmt.Sprintf("SELECT * FROM %s;", tableName)
		tableRows, err := st.DB.Query(query)
		if err != nil {
			return fmt.Errorf("querying table %s: %w", tableName, err)
		}

		cols, err := tableRows.Columns()
		if err != nil {
			tableRows.Close()
			return fmt.Errorf("getting columns for table %s: %w", tableName, err)
		}

		var tableData []map[string]interface{}
		for tableRows.Next() {
			values := make([]interface{}, len(cols))
			valuePtrs := make([]interface{}, len(cols))
			for i := range values {
				valuePtrs[i] = &values[i]
			}

			if err := tableRows.Scan(valuePtrs...); err != nil {
				tableRows.Close()
				return fmt.Errorf("scanning row in table %s: %w", tableName, err)
			}

			rowMap := make(map[string]interface{})
			for i, col := range cols {
				val := values[i]
				if b, ok := val.([]byte); ok {
					rowMap[col] = string(b)
				} else {
					rowMap[col] = val
				}
			}
			tableData = append(tableData, rowMap)
		}
		tableRows.Close()

		dbDump[tableName] = tableData
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("iterating tables: %w", err)
	}

	var sb strings.Builder
	if err := toml.NewEncoder(&sb).Encode(dbDump); err != nil {
		return fmt.Errorf("encoding TOML: %w", err)
	}

	// Make the output path absolute relative to the current directory.
	outputPath, err = filepath.Abs(outputPath)
	if err != nil {
		return err
	}

	if err := os.WriteFile(outputPath, []byte(sb.String()), 0644); err != nil {
		return fmt.Errorf("writing export file: %w", err)
	}

	return nil
}

// GetDBExportPath returns the full file path where the TOML dump will be saved.
// This example saves the file to ~/.config/lazaro/db_dump.toml.
func GetDBExportPath() (string, error) {
	home, err := os.UserHomeDir()
	if err != nil {
		return "", err
	}
	dir := filepath.Join(home, ".config", "lazaro")
	if err := os.MkdirAll(dir, 0755); err != nil {
		return "", err
	}
	return filepath.Join(dir, "db_dump.toml"), nil
}

// ImportDBFromTOML reads the TOML dump file at filePath and rebuilds the database
// by deleting current rows from all tables and then inserting the rows from the dump.
func ImportDBFromTOML(filePath string) error {
	data, err := os.ReadFile(filePath)
	if err != nil {
		return fmt.Errorf("Reading file %s: %w", filePath, err)
	}

	// The dump file is assumed to be a map from table names to an array of rows.
	var dbDump map[string][]map[string]interface{}
	if _, err := toml.Decode(string(data), &dbDump); err != nil {
		return fmt.Errorf("Decoding TOML: %w", err)
	}

	// Get a storage instance.
	st := NewStorage()
	db := st.DB

	// Begin a transaction.
	tx, err := db.Begin()
	if err != nil {
		return fmt.Errorf("Begin transaction: %w", err)
	}

	// Disable foreign keys for the duration of the import.
	if _, err := tx.Exec("PRAGMA foreign_keys = OFF;"); err != nil {
		tx.Rollback()
		return fmt.Errorf("Disabling foreign keys: %w", err)
	}

	// For each table in the dump:
	for table, rows := range dbDump {
		// Clear the table first.
		delQuery := fmt.Sprintf("DELETE FROM %s;", table)
		if _, err := tx.Exec(delQuery); err != nil {
			tx.Rollback()
			return fmt.Errorf("Clearing table %s: %w", table, err)
		}

		// Insert each row.
		for _, row := range rows {
			var columns []string
			var placeholders []string
			var values []interface{}
			for col, val := range row {
				columns = append(columns, col)
				placeholders = append(placeholders, "?")
				values = append(values, val)
			}
			query := fmt.Sprintf("INSERT INTO %s (%s) VALUES (%s);", table, strings.Join(columns, ", "), strings.Join(placeholders, ", "))
			if _, err := tx.Exec(query, values...); err != nil {
				tx.Rollback()
				return fmt.Errorf("Inserting into table %s: %w", table, err)
			}
		}
	}

	// Re-enable foreign keys.
	if _, err := tx.Exec("PRAGMA foreign_keys = ON;"); err != nil {
		tx.Rollback()
		return fmt.Errorf("Re-enabling foreign keys: %w", err)
	}

	// Commit the transaction.
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("Committing transaction: %w", err)
	}

	return nil
}
