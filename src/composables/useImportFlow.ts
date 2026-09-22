import { computed, onUnmounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";
import { ElMessage } from "element-plus";
import { listImportGuides, startImport } from "../api";
import type {
  ImportGuide,
  ImportMethodGuide,
  ImportJobView,
  ImportProgressEvent,
} from "../types";

export function useImportFlow(options: {
  onImported: () => Promise<void> | void;
}) {
  const { t } = useI18n();

  const importing = ref(false);
  const importDialogVisible = ref(false);
  const importWizardVisible = ref(false);
  const importWizardStep = ref<"source" | "method">("source");
  const importGuides = ref<ImportGuide[]>([]);
  const selectedImportGuide = ref<ImportGuide | null>(null);
  const importJobId = ref<string | null>(null);
  const importProgress = ref({
    phase: "pending",
    progress: 0,
    processed: 0,
    total: 0,
  });
  const importElapsedMs = ref(0);
  const importReportVisible = ref(false);
  const lastImportJob = ref<ImportJobView | null>(null);

  let importTimer: ReturnType<typeof setInterval> | null = null;
  let importUnlisten: UnlistenFn[] = [];

  const importPhaseLabel = computed(() => {
    const phase = importProgress.value.phase;
    if (
      phase === "extracting" ||
      phase === "parsing" ||
      phase === "persisting" ||
      phase === "done"
    ) {
      return t(`import.phase.${phase}`);
    }
    return t("import.phase.pending");
  });

  const importWizardTitle = computed(() => {
    if (importWizardStep.value === "source") {
      return t("import.selectSource");
    }
    return t("import.importFrom", {
      name: selectedImportGuide.value?.display_name ?? "",
    });
  });

  function formatImportElapsed(ms: number): string {
    const totalSeconds = Math.floor(ms / 1000);
    const minutes = Math.floor(totalSeconds / 60);
    const seconds = totalSeconds % 60;
    const time =
      minutes > 0
        ? t("import.elapsedMinutes", {
            m: minutes,
            s: String(seconds).padStart(2, "0"),
          })
        : t("import.elapsedSeconds", { n: totalSeconds });
    return t("import.elapsed", { time });
  }

  function startImportTimer() {
    stopImportTimer();
    importElapsedMs.value = 0;
    const startedAt = Date.now();
    importTimer = setInterval(() => {
      importElapsedMs.value = Date.now() - startedAt;
    }, 200);
  }

  function stopImportTimer() {
    if (importTimer !== null) {
      clearInterval(importTimer);
      importTimer = null;
    }
  }

  function cleanupImportListeners() {
    stopImportTimer();
    for (const unlisten of importUnlisten) {
      void unlisten();
    }
    importUnlisten = [];
  }

  onUnmounted(cleanupImportListeners);

  async function finishImport(job: ImportJobView) {
    importing.value = false;
    importDialogVisible.value = false;
    cleanupImportListeners();
    importJobId.value = null;
    lastImportJob.value = job;
    importReportVisible.value = true;

    if (job.status === "done" && job.result) {
      await options.onImported();
    }
  }

  async function startImportFlow(path: string, importerId?: string) {
    cleanupImportListeners();
    importing.value = true;
    importDialogVisible.value = true;
    startImportTimer();
    importProgress.value = {
      phase: "pending",
      progress: 0,
      processed: 0,
      total: 0,
    };

    try {
      const jobId = await startImport(path, importerId);
      importJobId.value = jobId;

      importUnlisten.push(
        await listen<ImportProgressEvent>("import-progress", (event) => {
          if (event.payload.job_id !== importJobId.value) {
            return;
          }
          importProgress.value = {
            phase: event.payload.phase,
            progress: Math.round(event.payload.progress * 100),
            processed: event.payload.processed,
            total: event.payload.total,
          };
        }),
      );

      importUnlisten.push(
        await listen<ImportJobView>("import-complete", (event) => {
          if (event.payload.id !== importJobId.value) {
            return;
          }
          void finishImport(event.payload);
        }),
      );
    } catch (error) {
      importing.value = false;
      importDialogVisible.value = false;
      cleanupImportListeners();
      ElMessage.error(String(error));
    }
  }

  async function openImportWizard() {
    importWizardStep.value = "source";
    selectedImportGuide.value = null;
    importWizardVisible.value = true;

    try {
      importGuides.value = await listImportGuides();
    } catch (error) {
      importWizardVisible.value = false;
      ElMessage.error(String(error));
    }
  }

  function importSupportStatusLabel(status: ImportGuide["support_status"]) {
    if (status === "experimental") return t("import.experimental");
    if (status === "beta") return t("import.beta");
    return t("import.stable");
  }

  function importSupportStatusType(
    status: ImportGuide["support_status"],
  ): "success" | "warning" | "info" {
    if (status === "experimental") return "warning";
    if (status === "beta") return "info";
    return "success";
  }

  async function importFromDetectedPath(path: string) {
    const guide = selectedImportGuide.value;
    if (!guide) {
      return;
    }
    importWizardVisible.value = false;
    await startImportFlow(path, guide.importer_id);
  }

  function selectImportSource(guide: ImportGuide) {
    selectedImportGuide.value = guide;
    importWizardStep.value = "method";
  }

  function backToImportSources() {
    importWizardStep.value = "source";
    selectedImportGuide.value = null;
  }

  async function pickImportPath(method: ImportMethodGuide) {
    const guide = selectedImportGuide.value;
    if (!guide) {
      return;
    }

    const isDirectory = method.kind === "directory";
    const extensions = (method.extensions ?? []).filter(Boolean);

    const selected = await open({
      multiple: false,
      directory: isDirectory,
      title: method.dialog_title,
      filters:
        !isDirectory && extensions.length > 0
          ? [{ name: method.label, extensions }]
          : undefined,
    });

    if (!selected || Array.isArray(selected)) {
      return;
    }

    importWizardVisible.value = false;
    await startImportFlow(selected, guide.importer_id);
  }

  return {
    importing,
    importDialogVisible,
    importWizardVisible,
    importWizardStep,
    importWizardTitle,
    importGuides,
    selectedImportGuide,
    importProgress,
    importPhaseLabel,
    importElapsedMs,
    formatImportElapsed,
    importReportVisible,
    lastImportJob,
    openImportWizard,
    importSupportStatusLabel,
    importSupportStatusType,
    importFromDetectedPath,
    selectImportSource,
    backToImportSources,
    pickImportPath,
  };
}
