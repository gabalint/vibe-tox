import { useState } from "react";
import { Select } from "@/components/Select";
import { type HelpArticle, pageForPath } from "./articles";
import { ManualPageView } from "./ManualPageView";
import styles from "./PageHelp.module.css";

export function HelpArticleView({ article, pathname }: { article: HelpArticle; pathname: string }) {
  const [pageTitle, setPageTitle] = useState(() => pageForPath(article, pathname)?.title);
  const page = article.pages.find((entry) => entry.title === pageTitle);
  return (
    <article className={styles.article}>
      <h3>{article.title}</h3>
      <p>{article.summary}</p>
      {article.pages.length > 1 && (
        <Select
          label="Page in this topic"
          name="manual-page"
          options={article.pages.map((entry) => ({ id: entry.title, label: entry.title }))}
          value={pageTitle ?? null}
          onChange={(value) => setPageTitle(value ?? undefined)}
        />
      )}
      {page && <ManualPageView key={page.title} page={page} />}
      <details open={article.pages.length === 0} className={styles.chapter}>
        <summary>Background and good practice</summary>
        {article.sections.map((section) => (
          <section key={section.title}>
            <h4>{section.title}</h4>
            {section.paragraphs.map((paragraph) => (
              <p key={paragraph}>{paragraph}</p>
            ))}
            {section.steps && (
              <ol>
                {section.steps.map((step) => (
                  <li key={step}>{step}</li>
                ))}
              </ol>
            )}
          </section>
        ))}
      </details>
      <footer className={styles.note}>
        <p>Software guidance. Follow your laboratory’s approved procedures for clinical work.</p>
        <p>
          Reviewed <time dateTime={article.reviewedOn}>{article.reviewedOn}</time> · Included with
          this app version
        </p>
      </footer>
    </article>
  );
}
