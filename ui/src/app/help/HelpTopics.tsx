import { useState } from "react";
import { Button } from "@/components/Button";
import { FindField } from "@/components/FindField";
import { articles, findArticles } from "./articles";
import styles from "./PageHelp.module.css";

export function HelpTopics({ onSelect }: { onSelect: (id: string) => void }) {
  const [query, setQuery] = useState("");
  const matches = findArticles(query);
  return (
    <div className={styles.topics}>
      <h3>All help topics</h3>
      <FindField
        label="Find help topics"
        placeholder="Search tasks or terms"
        value={query}
        onChange={setQuery}
        count={{ shown: matches.length, total: articles.length, noun: "topics", one: "topic" }}
      />
      {matches.length === 0 ? (
        <p>No help topics match. Try a page name or a shorter search.</p>
      ) : (
        <ul className={styles.topicList}>
          {matches.map((article) => (
            <li key={article.id}>
              <Button variant="quiet" onPress={() => onSelect(article.id)}>
                {article.title}
              </Button>
              <p>{article.summary}</p>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}
