package utils

import "time"

var SPLoc *time.Location

func init() {
	var err error
	SPLoc, err = time.LoadLocation("America/Sao_Paulo")
	if err != nil {
		panic("Failed to load São Paulo location " + err.Error())
	}
}

// FormatSaoPaulo returns the provided time formatted in São Paulo local time.
func FormatSaoPaulo(t time.Time) string {
	return t.In(SPLoc).Format(time.RFC1123)
}

// ToSaoPaulo converts a given time to São Paulo time.
func ToSaoPaulo(t time.Time) time.Time {
	return t.In(SPLoc)
}
