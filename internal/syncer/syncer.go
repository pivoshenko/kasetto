package syncer

import (
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/pivoshenko/sukiro/internal/config"
	"github.com/pivoshenko/sukiro/internal/hashing"
	"github.com/pivoshenko/sukiro/internal/source"
	"github.com/pivoshenko/sukiro/internal/state"
	"github.com/pivoshenko/sukiro/internal/util"
)

type Summary struct {
	Installed int `json:"installed"`
	Updated   int `json:"updated"`
	Removed   int `json:"removed"`
	Unchanged int `json:"unchanged"`
	Failed    int `json:"failed"`
}

type Action struct {
	Source   string `json:"source,omitempty"`
	Skill    string `json:"skill,omitempty"`
	Path     string `json:"path,omitempty"`
	Status   string `json:"status"`
	Hash     string `json:"hash,omitempty"`
	Error    string `json:"error,omitempty"`
	Revision string `json:"source_revision,omitempty"`
}

type Result struct {
	RunID       string   `json:"run_id"`
	Config      string   `json:"config"`
	Destination string   `json:"destination"`
	DryRun      bool     `json:"dry_run"`
	StartedAt   string   `json:"started_at"`
	FinishedAt  string   `json:"finished_at"`
	Summary     Summary  `json:"summary"`
	Actions     []Action `json:"actions"`
}

type Params struct {
	ConfigPath string
	Config     config.Config
	State      state.State
	DryRun     bool
	Quiet      bool
	RunID      string
}

func key(src, name string) string { return src + "::" + name }

func Run(p Params) (Result, state.State, error) {
	started := time.Now().UTC().Format(time.RFC3339)
	cfgDir := filepath.Dir(p.ConfigPath)
	destination := p.Config.Destination
	if !filepath.IsAbs(destination) {
		destination = filepath.Clean(filepath.Join(cfgDir, destination))
	}
	if !p.DryRun {
		_ = os.MkdirAll(destination, 0o755)
	}

	res := Result{RunID: p.RunID, Config: p.ConfigPath, Destination: destination, DryRun: p.DryRun, StartedAt: started}
	st := p.State
	if st.Skills == nil {
		st.Skills = map[string]state.SkillEntry{}
	}
	desired := map[string]bool{}

	for i, spec := range p.Config.Sources {
		stage := filepath.Join(os.TempDir(), fmt.Sprintf("skills-%s-%d", p.RunID, i+1))
		h, err := source.Create(spec, cfgDir, stage)
		if err != nil {
			res.Summary.Failed++
			res.Actions = append(res.Actions, Action{Source: spec.Source, Status: "source_error", Error: err.Error()})
			continue
		}
		targets := pickTargets(spec.Skills, h.Available)
		for _, t := range targets {
			desired[key(spec.Source, t.Name)] = true
			src := h.Available[t.Name]
			if t.Path != "" {
				cand := filepath.Join(h.RootDir, t.Path)
				if _, err := os.Stat(filepath.Join(cand, "SKILL.md")); err == nil {
					src = cand
				}
			}
			if src == "" {
				res.Summary.Failed++
				res.Actions = append(res.Actions, Action{Source: spec.Source, Skill: t.Name, Status: "not_found"})
				continue
			}
			hash, err := hashing.Dir(src)
			if err != nil {
				res.Summary.Failed++
				res.Actions = append(res.Actions, Action{Source: spec.Source, Skill: t.Name, Status: "failed", Error: err.Error()})
				continue
			}
			dst := filepath.Join(destination, t.Name)
			prev, hasPrev := st.Skills[key(spec.Source, t.Name)]
			if hasPrev && prev.Hash == hash && util.Exists(dst) {
				res.Summary.Unchanged++
				res.Actions = append(res.Actions, Action{Source: spec.Source, Skill: t.Name, Status: "unchanged", Hash: hash, Revision: h.Revision})
				continue
			}
			status := "installed"
			if hasPrev {
				status = "updated"
			}
			if p.DryRun {
				if hasPrev {
					status = "would_update"
				} else {
					status = "would_install"
				}
			} else {
				if err := util.CopyDir(src, dst); err != nil {
					res.Summary.Failed++
					res.Actions = append(res.Actions, Action{Source: spec.Source, Skill: t.Name, Status: "failed", Error: err.Error()})
					continue
				}
				st.Skills[key(spec.Source, t.Name)] = state.SkillEntry{Destination: dst, Hash: hash, Skill: t.Name, Source: spec.Source, SourceRevision: h.Revision, UpdatedAt: time.Now().UTC().Format(time.RFC3339)}
			}
			if status == "installed" || status == "would_install" {
				res.Summary.Installed++
			}
			if status == "updated" || status == "would_update" {
				res.Summary.Updated++
			}
			res.Actions = append(res.Actions, Action{Source: spec.Source, Skill: t.Name, Status: status, Hash: hash, Revision: h.Revision})
		}
	}

	for k, e := range st.Skills {
		if desired[k] {
			continue
		}
		status := "removed"
		if p.DryRun {
			status = "would_remove"
		} else {
			_ = os.RemoveAll(e.Destination)
			delete(st.Skills, k)
		}
		res.Summary.Removed++
		res.Actions = append(res.Actions, Action{Source: e.Source, Skill: e.Skill, Status: status, Path: e.Destination})
	}

	res.FinishedAt = time.Now().UTC().Format(time.RFC3339)
	return res, st, nil
}

func pickTargets(sf config.SkillsField, available map[string]string) []config.SkillTarget {
	if sf.Wildcard {
		out := make([]config.SkillTarget, 0, len(available))
		for k := range available {
			out = append(out, config.SkillTarget{Name: k})
		}
		return out
	}
	return sf.Items
}
