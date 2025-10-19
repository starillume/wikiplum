package main

import (
	"fmt"
	"html/template"
	"io"
	"io/fs"
	"os"
	"path/filepath"
	"strings"

	"github.com/yuin/goldmark"
)

const (
	ContentPath   = "content"
	BuildPath     = "build"
	TemplatesPath = "templates"
	BaseTemplate  = "base.html"
	PageTemplate  = "page.html"
	RootPage      = "index"
)

type PageData struct {
	Title   string
	HTML    template.HTML
	Sidebar []NavItem
	Rel     string
}

type NavItem struct {
	Title string
	Link  string
}

func main() {
	tmpl, err := mustLoadTemplates()
	if err != nil {
		fmt.Println("error loading templates: ", err)
		os.Exit(1)
	}

	if err := buildPages(tmpl); err != nil {
		fmt.Println("error building site: ", err)
		os.Exit(1)
	}

	if err := copyStatic(); err != nil {
		fmt.Println("error copying static files: ", err)
		os.Exit(1)
	}
}

func mustLoadTemplates() (*template.Template, error) {
	tmpl, err := template.ParseFiles(
		filepath.Join(TemplatesPath, BaseTemplate),
		filepath.Join(TemplatesPath, PageTemplate),
	)
	if err != nil {
		return nil, err
	}
	return tmpl, nil
}

func buildPages(tmpl *template.Template) error {
	return filepath.WalkDir(ContentPath, func(path string, d fs.DirEntry, err error) error {
		if err != nil || d.IsDir() || !strings.HasSuffix(path, ".md") {
			return err
		}

		outPath := outputPath(path)
		if err := os.MkdirAll(filepath.Dir(outPath), 0755); err != nil {
			return err
		}

		html, err := renderMarkdown(path)
		if err != nil {
			return err
		}

		rel, err := filepath.Rel(filepath.Dir(path), ContentPath)
		if err != nil {
			return err
		}

		data := PageData{
			Title:   filepath.Base(strings.TrimSuffix(path, ".md")),
			HTML:    template.HTML(html),
			Sidebar: generateSidebar(ContentPath, path),
			Rel:     rel,
		}

		return writePage(outPath, tmpl, data)
	})
}

func outputPath(mdPath string) string {
	rel, _ := filepath.Rel(ContentPath, mdPath)
	rel = strings.TrimSuffix(rel, ".md")
	return filepath.Join(BuildPath, rel+".html")
}

func mdLinkToHTML(md []byte) []byte {
	return []byte(strings.ReplaceAll(string(md), ".md", ".html"))
}

func renderMarkdown(mdPath string) (string, error) {
	md, err := os.ReadFile(mdPath)
	if err != nil {
		return "", err
	}

	var html strings.Builder
	if err := goldmark.Convert(mdLinkToHTML(md), &html); err != nil {
		return "", err
	}
	return html.String(), nil
}

func writePage(outPath string, tmpl *template.Template, data PageData) error {
	f, err := os.Create(outPath)
	if err != nil {
		return err
	}
	defer f.Close()

	return tmpl.ExecuteTemplate(f, BaseTemplate, data)
}

func copyStatic() error {
	src := "static"
	dst := filepath.Join(BuildPath, "static")
	if err := os.MkdirAll(dst, 0755); err != nil {
		return err
	}

	return filepath.Walk(src, func(path string, info os.FileInfo, err error) error {
		if err != nil || info.IsDir() {
			return err
		}

		rel, _ := filepath.Rel(src, path)
		outPath := filepath.Join(dst, rel)

		return copyFile(path, outPath)
	})
}

func copyFile(src string, dst string) error {
	in, err := os.Open(src)
	if err != nil {
		return err
	}
	defer in.Close()

	out, err := os.Create(dst)
	if err != nil {
		return err
	}
	defer out.Close()

	_, err = io.Copy(out, in)
	return err
}

func generateSidebar(root string, currentPath string) []NavItem {
    var items []NavItem
    filepath.WalkDir(root, func(path string, d fs.DirEntry, err error) error {
        if err != nil || d.IsDir() || strings.Contains(path, RootPage) || !strings.HasSuffix(path, ".md") {
            return err
        }

        relRoot, _ := filepath.Rel(filepath.Dir(currentPath), root)
        rel, _ := filepath.Rel(root, path)

        var link string
        if relRoot == "." {
            link = rel
        } else {
            link = filepath.Join(relRoot, rel)
        }

        link = strings.TrimSuffix(link, ".md")

        items = append(items, NavItem{
            Title: filepath.Base(link),
            Link:  link + ".html",
        })
        return nil
    })
    return items
}
