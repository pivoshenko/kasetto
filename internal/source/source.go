package source

import (
	"archive/tar"
	"compress/gzip"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"regexp"
	"strings"

	"github.com/pivoshenko/sukiro/internal/config"
)

type Handle struct {
	SourceRaw string
	RootDir   string
	Revision  string
	Available map[string]string
}

func Create(spec config.SourceSpec, cfgDir, stageDir string) (Handle, error) {
	if strings.Contains(spec.Source, "://") {
		return remote(spec, stageDir)
	}
	root := resolve(spec.Source, cfgDir)
	if fi, err := os.Stat(root); err != nil || !fi.IsDir() {
		return Handle{}, fmt.Errorf("local source missing: %s", root)
	}
	return Handle{SourceRaw: spec.Source, RootDir: root, Revision: "local", Available: discover(root)}, nil
}

func resolve(p, base string) string {
	if filepath.IsAbs(p) {
		return p
	}
	return filepath.Clean(filepath.Join(base, p))
}

func discover(root string) map[string]string {
	out := map[string]string{}
	candidates := []string{root, filepath.Join(root, "skills")}
	for _, c := range candidates {
		ents, _ := os.ReadDir(c)
		for _, e := range ents {
			if !e.IsDir() {
				continue
			}
			d := filepath.Join(c, e.Name())
			if _, err := os.Stat(filepath.Join(d, "SKILL.md")); err == nil {
				out[e.Name()] = d
			}
		}
	}
	return out
}

func remote(spec config.SourceSpec, stageDir string) (Handle, error) {
	owner, repo, err := parseGitHub(spec.Source)
	if err != nil {
		return Handle{}, err
	}
	branch := spec.Branch
	if branch == "" {
		branch = "main"
	}
	url := fmt.Sprintf("https://codeload.github.com/%s/%s/tar.gz/refs/heads/%s", owner, repo, branch)
	if err := downloadAndExtract(url, stageDir); err != nil {
		if spec.Branch == "" {
			branch = "master"
			url = fmt.Sprintf("https://codeload.github.com/%s/%s/tar.gz/refs/heads/%s", owner, repo, branch)
			if err2 := downloadAndExtract(url, stageDir); err2 != nil {
				return Handle{}, err
			}
		} else {
			return Handle{}, err
		}
	}
	return Handle{SourceRaw: spec.Source, RootDir: stageDir, Revision: "branch:" + branch, Available: discover(stageDir)}, nil
}

func parseGitHub(url string) (string, string, error) {
	r := regexp.MustCompile(`^https?://github\.com/([^/]+)/([^/]+?)(?:\.git)?/?$`)
	m := r.FindStringSubmatch(url)
	if len(m) != 3 {
		return "", "", fmt.Errorf("unsupported remote source: %s", url)
	}
	return m[1], m[2], nil
}

func downloadAndExtract(url, dst string) error {
	resp, err := http.Get(url) //nolint:gosec
	if err != nil {
		return err
	}
	defer resp.Body.Close()
	if resp.StatusCode >= 300 {
		return fmt.Errorf("download failed: %s", resp.Status)
	}
	_ = os.RemoveAll(dst)
	if err := os.MkdirAll(dst, 0o755); err != nil {
		return err
	}
	gz, err := gzip.NewReader(resp.Body)
	if err != nil {
		return err
	}
	defer gz.Close()
	tr := tar.NewReader(gz)
	for {
		h, err := tr.Next()
		if err == io.EOF {
			break
		}
		if err != nil {
			return err
		}
		parts := strings.Split(h.Name, "/")
		if len(parts) < 2 {
			continue
		}
		rel := filepath.Join(parts[1:]...)
		if strings.Contains(rel, "..") {
			return fmt.Errorf("unsafe archive path")
		}
		target := filepath.Join(dst, rel)
		if h.FileInfo().IsDir() {
			_ = os.MkdirAll(target, 0o755)
			continue
		}
		if err := os.MkdirAll(filepath.Dir(target), 0o755); err != nil {
			return err
		}
		f, err := os.Create(target)
		if err != nil {
			return err
		}
		if _, err := io.Copy(f, tr); err != nil {
			_ = f.Close()
			return err
		}
		_ = f.Close()
	}
	return nil
}
