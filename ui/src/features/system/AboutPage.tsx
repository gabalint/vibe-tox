import { ErrorState } from "@/components/ErrorState";
import { Facts } from "@/components/Facts";
import { LoadingState } from "@/components/LoadingState";
import { Page, PageHeader } from "@/components/Page";
import type { AppInfo } from "@/generated/AppInfo";
import { useCommandQuery } from "@/lib/commands";

/** "About this app", from the user menu: the app and web engine versions to quote to support (NFR-33). */
export function AboutPage() {
  return (
    <Page>
      <PageHeader
        title="About this app"
        description="Versions of this app and of the operating system's web engine, to quote to support."
      />
      <SystemInfo />
    </Page>
  );
}

function SystemInfo() {
  const info = useCommandQuery<AppInfo>("app_info");
  if (info.isPending) return <LoadingState />;
  if (info.isError) {
    return <ErrorState error={info.error} onRetry={() => void info.refetch()} />;
  }
  return (
    <Facts
      layout="compact"
      items={[
        { label: "App version", value: info.data.appVersion },
        {
          label: "Web engine version",
          value: info.data.webviewVersion ?? "Not reported by the operating system",
        },
        { label: "Platform", value: info.data.platform },
      ]}
    />
  );
}
