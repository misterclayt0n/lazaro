package utils

func CalculateEpley1RM(weight float32, reps int) float32 {
	if reps == 0 {
		return 0
	}

	return weight * (1 + float32(reps) / 30)
}
