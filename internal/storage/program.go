package storage

import (
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"time"

	"github.com/BurntSushi/toml"
	"github.com/google/uuid"
	"github.com/misterclayt0n/lazaro/internal/models"
)

func (s *Storage) CreateProgram(tomlData []byte) error {
	ctx := context.Background()
	tx, err := s.DB.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("Failed to begin transaction: %w", err)
	}
	defer tx.Rollback()

	// Parse TOML.
	var programTOML models.ProgramTOML
	if err := toml.Unmarshal(tomlData, &programTOML); err != nil {
		return fmt.Errorf("Invalid TOML format: %w", err)
	}

	// Create main program.
	programID := uuid.New().String()
	createdAt := time.Now().UTC().Format(time.RFC3339)
	_, err = tx.ExecContext(ctx,
		`INSERT INTO programs (id, name, description, created_at)
         VALUES (?, ?, ?, ?)`,
		programID,
		programTOML.Name,
		programTOML.Description,
		createdAt,
	)
	if err != nil {
		return fmt.Errorf("Failed to create program: %w", err)
	}

	// Determine if we have week information.
	if len(programTOML.Weeks) > 0 {
		for _, week := range programTOML.Weeks {
			for _, blockTOML := range week.Blocks {
				// Insert the block including the week number.
				blockID := uuid.New().String()
				_, err = tx.ExecContext(ctx,
					`INSERT INTO program_blocks
                     (id, program_id, name, description, week)
                     VALUES (?, ?, ?, ?, ?)`,
					blockID,
					programID,
					blockTOML.Name,
					blockTOML.Description,
					week.Week,
				)
				if err != nil {
					return fmt.Errorf("Failed to create program block: %w", err)
				}

				// Process exercises in the block.
				if err := insertProgramExercises(ctx, tx, blockID, blockTOML.Exercises); err != nil {
					return err
				}
			}
		}
	} else {
		// Fall back to the legacy structure.
		// NOTE: This is when no weeks are provided.
		for _, blockTOML := range programTOML.Blocks {
			blockID := uuid.New().String()
			_, err = tx.ExecContext(ctx,
				`INSERT INTO program_blocks
                 (id, program_id, name, description, week)
                 VALUES (?, ?, ?, ?, ?)`,
				blockID,
				programID,
				blockTOML.Name,
				blockTOML.Description,
				nil, // No week information provided.
			)
			if err != nil {
				return fmt.Errorf("Failed to create program block: %w", err)
			}
			if err := insertProgramExercises(ctx, tx, blockID, blockTOML.Exercises); err != nil {
				return err
			}
		}
	}

	if err := tx.Commit(); err != nil {
		return fmt.Errorf("Failed to commit transaction: %w", err)
	}
	return nil
}

func (s *Storage) ListPrograms() ([]models.Program, error) {
	rows, err := s.DB.Query(`
        SELECT id, name, description, created_at
        FROM programs
    `)
	if err != nil {
		return nil, fmt.Errorf("Failed to query programs: %w", err)
	}
	defer rows.Close()

	var programs []models.Program
	for rows.Next() {
		var p models.Program
		var createdAt string

		err := rows.Scan(
			&p.ID,
			&p.Name,
			&p.Description,
			&createdAt,
		)
		if err != nil {
			return nil, fmt.Errorf("Failed to scan program: %w", err)
		}

		p.CreatedAt, _ = time.Parse(time.RFC3339, createdAt)
		programs = append(programs, p)
	}

	return programs, nil
}

// UpdateProgram updates the existing program based on a TOML file.
// It updates only the program and block/exercise fields so that existing sessions are not lost.
// This function kind of makes fun of clean code, and I'm all here for it.
func (s *Storage) UpdateProgram(tomlData []byte) error {
	// Parse the TOML file into a ProgramTOML structure.
	var progTOML models.ProgramTOML
	if err := toml.Unmarshal(tomlData, &progTOML); err != nil {
		return fmt.Errorf("Invalid TOML format: %w", err)
	}

	// Retrieve the existing program by name.
	// NOTE: This assumes program name is unique.
	existingProgram, err := s.GetProgramByName(progTOML.Name)
	if err != nil {
		return fmt.Errorf("Failed to get existing program: %w", err)
	}

	ctx := context.Background()
	tx, err := s.DB.BeginTx(ctx, nil)
	if err != nil {
		return fmt.Errorf("Failed to begin transaction: %w", err)
	}
	// Roll back on error.
	defer tx.Rollback()

	// Update the program’s description if it has changed.
	if existingProgram.Description != progTOML.Description {
		_, err := tx.ExecContext(ctx, `UPDATE programs SET description = ? WHERE id = ?`,
			progTOML.Description, existingProgram.ID)
		if err != nil {
			return fmt.Errorf("Failed to update program: %w", err)
		}
	}

	// If the TOML file includes weeks, use that branch.
	if len(progTOML.Weeks) > 0 {
		for _, weekTOML := range progTOML.Weeks {
			for _, newBlock := range weekTOML.Blocks {
				var blockID string
				// Look for a block with the given name AND week.
				err := tx.QueryRowContext(ctx,
					`SELECT id FROM program_blocks WHERE program_id = ? AND name = ? AND week = ?`,
					existingProgram.ID, newBlock.Name, weekTOML.Week,
				).Scan(&blockID)
				if err != nil {
					if err == sql.ErrNoRows {
						// No such block exists; insert a new one.
						blockID = generateID()
						_, err = tx.ExecContext(ctx,
							`INSERT INTO program_blocks (id, program_id, name, description, week)
                             VALUES (?, ?, ?, ?, ?)`,
							blockID, existingProgram.ID, newBlock.Name, newBlock.Description, weekTOML.Week,
						)
						if err != nil {
							return fmt.Errorf("Failed to insert new block: %w", err)
						}
					} else {
						return fmt.Errorf("Failed to query program block: %w", err)
					}
				} else {
					// Block exists: update its description (if necessary).
					_, err = tx.ExecContext(ctx,
						`UPDATE program_blocks SET description = ? WHERE id = ?`,
						newBlock.Description, blockID,
					)
					if err != nil {
						return fmt.Errorf("Failed to update block: %w", err)
					}
				}

				// Process exercises for this block.
				for _, newEx := range newBlock.Exercises {
					var exerciseID string
					err := tx.QueryRowContext(ctx,
						"SELECT id FROM exercises WHERE name = ?",
						newEx.Name,
					).Scan(&exerciseID)
					if err != nil {
						if err == sql.ErrNoRows {
							return fmt.Errorf("Exercise '%s' not found", newEx.Name)
						}
						return fmt.Errorf("Failed to query exercise: %w", err)
					}

					// Marshal JSON for the reps and target values.
					repsJSON, err := json.Marshal(newEx.Reps)
					if err != nil {
						return fmt.Errorf("Failed to marshal reps: %w", err)
					}
					targetRPEJSON, err := json.Marshal(newEx.TargetRPE)
					if err != nil {
						return fmt.Errorf("Failed to marshal target_rpe: %w", err)
					}
					targetRMPercentJSON, err := json.Marshal(newEx.TargetRMPercent)
					if err != nil {
						return fmt.Errorf("Failed to marshal target_rm_percent: %w", err)
					}

					// Check if a program_exercise for this exercise already exists in this block.
					var peID string
					err = tx.QueryRowContext(ctx,
						`SELECT id FROM program_exercises
						 WHERE program_block_id = ? AND exercise_id = ?`,
						blockID, exerciseID,
					).Scan(&peID)
					if err != nil {
						if err == sql.ErrNoRows {
							// Insert a new program exercise.
							peID = generateID()
							_, err = tx.ExecContext(ctx,
								`INSERT INTO program_exercises
								(id, program_block_id, exercise_id, sets, reps, target_rpe, target_rm_percent, notes, program_1rm, technique, technique_group)
								VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
								peID, blockID, exerciseID, newEx.Sets, string(repsJSON),
								string(targetRPEJSON), string(targetRMPercentJSON), newEx.ProgramNotes, newEx.Program1RM,
							)
							if err != nil {
								return fmt.Errorf("Failed to insert program exercise: %w", err)
							}
						} else {
							return fmt.Errorf("Failed to query program exercise: %w", err)
						}
					} else {
						// Update the existing program exercise.
						_, err = tx.ExecContext(ctx,
							`UPDATE program_exercises SET sets = ?, reps = ?, target_rpe = ?, target_rm_percent = ?, notes = ?, program_1rm = ?, technique = ?, technique_group = ?
							 WHERE id = ?`,
							newEx.Sets, string(repsJSON), string(targetRPEJSON), string(targetRMPercentJSON), newEx.ProgramNotes, newEx.Program1RM, newEx.Technique, newEx.TechniqueGroup,
							peID,
						)
						if err != nil {
							return fmt.Errorf("Failed to update program exercise: %w", err)
						}
					}
				} // end for each exercise in the block
			} // end for each block in a week
		} // end for each week
	} else {
		// Fallback to legacy update for programs that do not use weeks.
		for _, newBlock := range progTOML.Blocks {
			var blockID string
			err := tx.QueryRowContext(ctx,
				`SELECT id FROM program_blocks WHERE program_id = ? AND name = ?`,
				existingProgram.ID, newBlock.Name,
			).Scan(&blockID)
			if err != nil {
				if err == sql.ErrNoRows {
					blockID = generateID()
					_, err = tx.ExecContext(ctx,
						`INSERT INTO program_blocks (id, program_id, name, description, week)
                         VALUES (?, ?, ?, ?, ?)`,
						blockID, existingProgram.ID, newBlock.Name, newBlock.Description, nil,
					)
					if err != nil {
						return fmt.Errorf("Failed to insert new block: %w", err)
					}
				} else {
					return fmt.Errorf("Failed to query program block: %w", err)
				}
			} else {
				_, err = tx.ExecContext(ctx,
					`UPDATE program_blocks SET description = ? WHERE id = ?`,
					newBlock.Description, blockID,
				)
				if err != nil {
					return fmt.Errorf("Failed to update block: %w", err)
				}
			}

			// Process exercises in the block.
			for _, newEx := range newBlock.Exercises {
				var exerciseID string
				err := tx.QueryRowContext(ctx,
					"SELECT id FROM exercises WHERE name = ?",
					newEx.Name,
				).Scan(&exerciseID)
				if err != nil {
					if err == sql.ErrNoRows {
						return fmt.Errorf("Exercise '%s' not found", newEx.Name)
					}
					return fmt.Errorf("Failed to query exercise: %w", err)
				}

				repsJSON, err := json.Marshal(newEx.Reps)
				if err != nil {
					return fmt.Errorf("Failed to marshal reps: %w", err)
				}
				targetRPEJSON, err := json.Marshal(newEx.TargetRPE)
				if err != nil {
					return fmt.Errorf("Failed to marshal target_rpe: %w", err)
				}
				targetRMPercentJSON, err := json.Marshal(newEx.TargetRMPercent)
				if err != nil {
					return fmt.Errorf("Failed to marshal target_rm_percent: %w", err)
				}

				var peID string
				err = tx.QueryRowContext(ctx,
					"SELECT id FROM program_exercises WHERE program_block_id = ? AND exercise_id = ?",
					blockID, exerciseID,
				).Scan(&peID)
				if err != nil {
					if err == sql.ErrNoRows {
						peID = generateID()
						_, err = tx.ExecContext(ctx,
							`INSERT INTO program_exercises
                             (id, program_block_id, exercise_id, sets, reps, target_rpe, target_rm_percent, notes, program_1rm, technique, technique_group)
                             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`,
							peID, blockID, exerciseID, newEx.Sets, string(repsJSON),
							string(targetRPEJSON), string(targetRMPercentJSON), newEx.ProgramNotes, newEx.Program1RM, newEx.Technique, newEx.TechniqueGroup,
						)
						if err != nil {
							return fmt.Errorf("Failed to insert program exercise: %w", err)
						}
					} else {
						return fmt.Errorf("Failed to query program exercise: %w", err)
					}
				} else {
					_, err = tx.ExecContext(ctx,
						`UPDATE program_exercises SET sets = ?, reps = ?, target_rpe = ?, target_rm_percent = ?, notes = ?, program_1rm = ?, technique = ?, technique_group = ?
                         WHERE id = ?`,
						newEx.Sets, string(repsJSON), string(targetRPEJSON), string(targetRMPercentJSON), newEx.ProgramNotes, newEx.Program1RM, newEx.Technique, newEx.TechniqueGroup, peID,
					)
					if err != nil {
						return fmt.Errorf("Failed to update program exercise: %w", err)
					}
				}
			} // end for each exercise
		} // end for each legacy block
	}

	// Commit the transaction.
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("Failed to commit transaction: %w", err)
	}

	return nil
}

func (s *Storage) DeleteProgramByName(name string) error {
	ctx := context.Background()

	// First, find the program ID by name.
	var programID string
	err := s.DB.QueryRowContext(ctx, `SELECT id FROM programs WHERE name = ?`, name).Scan(&programID)
	if err != nil {
		return fmt.Errorf("Program not found: %w", err)
	}

	// Delete the program row.
	_, err = s.DB.ExecContext(ctx, `DELETE FROM programs WHERE id = ?`, programID)
	if err != nil {
		return fmt.Errorf("Failed to delete program: %w", err)
	}

	return nil
}

func generateID() string {
	return uuid.New().String()
}

func insertProgramExercises(ctx context.Context, tx *sql.Tx, blockID string, exercises []models.ExerciseTOML) error {
	for index, exerciseTOML := range exercises {
		// Get the exercise ID from the exercises table.
		var exerciseID string
		err := tx.QueryRowContext(ctx, "SELECT id FROM exercises WHERE name = ?", exerciseTOML.Name).Scan(&exerciseID)
		if err != nil {
			if err == sql.ErrNoRows {
				return fmt.Errorf("exercise '%s' not found", exerciseTOML.Name)
			}
			return fmt.Errorf("Failed to validate exercise: %w", err)
		}

		repsJSON, err := json.Marshal(exerciseTOML.Reps)
		if err != nil {
			return fmt.Errorf("Failed to marshal reps: %w", err)
		}
		targetRPEJSON, err := json.Marshal(exerciseTOML.TargetRPE)
		if err != nil {
			return fmt.Errorf("Failed to marshal target_rpe: %w", err)
		}
		targetRMPercentJSON, err := json.Marshal(exerciseTOML.TargetRMPercent)
		if err != nil {
			return fmt.Errorf("Failed to marshal target_rm_percent: %w", err)
		}
		optionsJSON, err := json.Marshal(exerciseTOML.Options)
		if err != nil {
			return fmt.Errorf("Failed to marshal options: %w", err)
		}

		_, err = tx.ExecContext(ctx,
			`INSERT INTO program_exercises
        (id, program_block_id, exercise_id, sets, reps, target_rpe, target_rm_percent, notes, program_1rm, options, technique, technique_group, order_index)
     VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
     ON CONFLICT(program_block_id, exercise_id) DO UPDATE SET
         sets = excluded.sets,
         reps = excluded.reps,
         target_rpe = excluded.target_rpe,
         target_rm_percent = excluded.target_rm_percent,
         notes = excluded.notes,
         program_1rm = excluded.program_1rm,
         options = excluded.options,
         technique = excluded.technique,
         technique_group = excluded.technique_group,
         order_index = excluded.order_index`,
			uuid.New().String(),
			blockID,
			exerciseID,
			exerciseTOML.Sets,
			string(repsJSON),
			string(targetRPEJSON),
			string(targetRMPercentJSON),
			exerciseTOML.ProgramNotes,
			exerciseTOML.Program1RM,
			string(optionsJSON),
			exerciseTOML.Technique,
			exerciseTOML.TechniqueGroup,
			index, // this is the order index (starting at 0 or 1)
		)
		if err != nil {
			return fmt.Errorf("Failed to create program exercise: %w", err)
		}
	}
	return nil
}

// GetProgramNameForSession returns the program name for the given training session ID.
func (s *Storage) GetProgramNameForSession(sessionID string) (string, error) {
	var programName string
	query := `
      SELECT p.name
      FROM training_sessions ts
      JOIN program_blocks pb ON ts.program_block_id = pb.id
      JOIN programs p ON pb.program_id = p.id
      WHERE ts.id = ?
    `
	err := s.DB.QueryRow(query, sessionID).Scan(&programName)
	if err != nil {
		return "", err
	}
	return programName, nil
}
