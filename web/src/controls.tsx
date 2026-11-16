// The controls, drawn by Preact. Each reads the signals it shows, so a change redraws that control alone, and each acts
// on the viewer only through the actions it is given.

import type { Report } from "./report.ts";
import { failure, notice, selection, status } from "./state.ts";

export interface Actions {
  fit(): void;
  centreOnSelection(): void;
}

function megabytes(bytes: number): string {
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function Status() {
  const { faces, triangles, pending, fallback } = status.value;
  const uploading = pending > 0 ? `, ${megabytes(pending)} to upload` : "";
  return (
    <span class="status">
      {faces} faces, {triangles} triangles{uploading}{fallback ? ", on a software adapter" : ""}
    </span>
  );
}

function Controls({ actions }: { actions: Actions }) {
  return (
    <div class="controls" role="toolbar" aria-label="View">
      <button type="button" onClick={() => actions.fit()} title="Fit the model to the window">Fit</button>
      <button
        type="button"
        onClick={() => actions.centreOnSelection()}
        disabled={selection.value.length === 0}
        title="Centre the view on the selection"
      >
        Centre
      </button>
      <Status />
    </div>
  );
}

function Shown({ shown, kind }: { shown: Report; kind: string }) {
  return (
    <section class={`report ${kind}`} role="alert" data-code={shown.code} data-remedy={shown.remedy?.kind}>
      <p class="code">{shown.code}</p>
      <p class="message">{shown.message}</p>
      {shown.remedy && <p class="remedy">{shown.remedy.text}</p>}
    </section>
  );
}

/** The controls over the drawing, or, when the page cannot draw, the report in its place. */
export function App({ actions }: { actions: Actions | null }) {
  const failed = failure.value;
  if (failed) return <Shown shown={failed} kind="failure" />;
  const shown = notice.value;
  return (
    <>
      {actions && <Controls actions={actions} />}
      {shown && <Shown shown={shown} kind="notice" />}
    </>
  );
}
