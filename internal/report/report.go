package report

import (
	"encoding/json"
	"os"
	"path/filepath"

	"github.com/pivoshenko/sukiro/internal/syncer"
)

func SaveDefault(runID string, r syncer.Result) (string, error) {
	home, _ := os.UserHomeDir()
	dir := filepath.Join(home, ".ai", "bootstrap", "runs", "run-"+runID)
	if err := os.MkdirAll(dir, 0o755); err != nil {
		return "", err
	}
	p := filepath.Join(dir, "report.json")
	b, _ := json.MarshalIndent(r, "", "  ")
	if err := os.WriteFile(p, b, 0o644); err != nil {
		return "", err
	}
	return p, nil
}
