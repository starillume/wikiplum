package main

import (
	"encoding/json"
	"net/http"
	"os"
	"strings"

	"github.com/syumai/workers"
	"gopkg.in/yaml.v3"
)

func apiHandler(w http.ResponseWriter, r *http.Request) {
	path := strings.TrimPrefix(r.URL.Path, "/api/")
	if path == "" {
		http.Error(w, "not found", 404)
		return
	}

	mdPath := "content/" + path + ".md"
	data, err := os.ReadFile(mdPath)
	if err != nil {
		http.Error(w, "file not found", 404)
		return
	}

	fm := parseFrontmatter(data)
	if fm == nil {
		fm = map[string]string{}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(fm)
}

// frontmatter is the yaml metadata in the beginning of md files
func parseFrontmatter(md []byte) map[string]string {
	content := strings.TrimSpace(string(md))
	if !strings.HasPrefix(content, "---") {
		return nil
	}

	parts := strings.SplitN(content, "---", 3)
	if len(parts) < 3 {
		return nil
	}

	yamlPart := strings.TrimSpace(parts[1])

	var fm map[string]string
	if err := yaml.Unmarshal([]byte(yamlPart), &fm); err != nil {
		return nil
	}

	return fm
}

func main() {
	http.Handle("/", http.HandlerFunc(apiHandler))
	workers.Serve(nil)
}
