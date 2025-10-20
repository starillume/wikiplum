package main

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"strings"

	"github.com/syumai/workers"
	"github.com/syumai/workers/cloudflare/fetch"
	"gopkg.in/yaml.v3"
)

const REPO_URL = "https://raw.githubusercontent.com/starillume/wikiplum/refs/heads/%s/content/%s"
const USER_AGENT = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10.15; rv:109.0) Gecko/20100101 Firefox/111.0"
const FILE_NOT_FOUND_ERROR_MESSAGE = "file not found"

func fetchWikiPage(ctx context.Context, path string, branch string) ([]byte, error) {
	cli := fetch.NewClient()

	url := fmt.Sprintf(REPO_URL, branch, path)

	r, err := fetch.NewRequest(ctx, http.MethodGet, url, nil)
	if err != nil {
		return nil, err
	}
	r.Header.Set("User-Agent", USER_AGENT)

	res, err := cli.Do(r, nil)
	if err != nil {
		return nil, err
	}

	if res.StatusCode == http.StatusNotFound {
		return nil, errors.New(FILE_NOT_FOUND_ERROR_MESSAGE)
	}

	buf := new(bytes.Buffer)

	io.Copy(buf, res.Body)

	return buf.Bytes(), nil
}

func apiHandler(w http.ResponseWriter, r *http.Request) {
	path := strings.TrimPrefix(r.URL.Path, "/api/")
	if path == "" {
		http.Error(w, "not found", 404)
		return
	}

	branch := r.URL.Query().Get("branch")
	if branch == "" {
		branch = "main"
	}

	mdPath := path + ".md"
	data, err := fetchWikiPage(r.Context(), mdPath, branch)
	if err != nil {
		if err.Error() == FILE_NOT_FOUND_ERROR_MESSAGE {
			http.Error(w, FILE_NOT_FOUND_ERROR_MESSAGE, 404)
			return
		}

		fmt.Printf("error: %+v\n", err)

		http.Error(w, "internal error", 500)
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
