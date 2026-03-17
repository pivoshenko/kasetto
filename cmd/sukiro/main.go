package main

import (
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/pivoshenko/sukiro/internal/config"
	"github.com/pivoshenko/sukiro/internal/report"
	"github.com/pivoshenko/sukiro/internal/state"
	"github.com/pivoshenko/sukiro/internal/syncer"
)

func main() {
	var cfgPath string
	var dryRun bool
	var quiet bool
	var jsonOut bool

	flag.StringVar(&cfgPath, "config", "skills.config.yaml", "Path to skills config")
	flag.BoolVar(&dryRun, "dry-run", false, "Resolve and compare without writing")
	flag.BoolVar(&quiet, "quiet", false, "Quiet output")
	flag.BoolVar(&jsonOut, "json", false, "Print JSON summary")
	flag.Parse()

	absCfg, _ := filepath.Abs(cfgPath)
	cfg, err := config.Load(absCfg)
	if err != nil {
		exitErr(err)
	}

	st, err := state.LoadDefault()
	if err != nil {
		exitErr(err)
	}

	runID := time.Now().UTC().Format("20060102T150405Z")
	res, nextState, err := syncer.Run(syncer.Params{
		ConfigPath: absCfg,
		Config:     cfg,
		State:      st,
		DryRun:     dryRun,
		Quiet:      quiet,
		RunID:      runID,
	})
	if err != nil {
		exitErr(err)
	}

	if !dryRun {
		nextState.LastRun = time.Now().UTC().Format(time.RFC3339)
		if err := state.SaveDefault(nextState); err != nil {
			exitErr(err)
		}
	}

	reportPath, err := report.SaveDefault(runID, res)
	if err != nil {
		exitErr(err)
	}

	if jsonOut {
		_ = json.NewEncoder(os.Stdout).Encode(res)
	} else if !quiet {
		fmt.Printf("Summary: installed=%d updated=%d removed=%d unchanged=%d failed=%d\n", res.Summary.Installed, res.Summary.Updated, res.Summary.Removed, res.Summary.Unchanged, res.Summary.Failed)
		fmt.Printf("Report: %s\n", reportPath)
	}

	if res.Summary.Failed > 0 {
		os.Exit(1)
	}
}

func exitErr(err error) {
	fmt.Fprintf(os.Stderr, "sukiro: %v\n", err)
	os.Exit(2)
}
