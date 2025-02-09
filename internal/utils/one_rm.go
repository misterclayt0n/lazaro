package utils

func CalculateEpley1RM(weight float32, reps int) float32 {
	if reps == 0 {
		return 0
	}

	return weight * (1 + float32(reps)/30)
}

func CalculateInitialOneRM() float32 {
	// For new exercises, I probably should just default to 0.
	// NOTE: I may change this behabior somehow, hence why this function exists
	return 0
}
