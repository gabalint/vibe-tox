import { useState } from "react";
import { useForm } from "react-hook-form";
import { Button } from "@/components/Button";
import { ConfirmDialog } from "@/components/ConfirmDialog";
import { Dialog } from "@/components/Dialog";
import { FormDialog } from "@/components/FormDialog";
import { PasswordField } from "@/components/PasswordField";
import { ReasonField } from "@/components/ReasonField";
import { useToast } from "@/components/Toast";
import { Toolbar } from "@/components/Toolbar";
import { GallerySection } from "./GallerySection";
import styles from "./gallery.module.css";

export function ActionsSection() {
  const [dialogOpen, setDialogOpen] = useState(false);
  const [confirmOpen, setConfirmOpen] = useState(false);
  const [formOpen, setFormOpen] = useState(false);
  const toast = useToast();
  const form = useForm({ defaultValues: { reason: "", password: "" } });

  return (
    <GallerySection title="Buttons, dialogs and notices">
      <div className={styles.row}>
        <Button variant="primary">Primary</Button>
        <Button>Secondary</Button>
        <Button variant="danger">Withdraw report</Button>
        <Button variant="quiet">Quiet</Button>
        <Button isDisabled>Disabled</Button>
        <Button isPending>Pending</Button>
      </div>
      <div className={styles.row}>
        <Button onPress={() => setDialogOpen(true)}>Open dialog</Button>
        <Button onPress={() => setConfirmOpen(true)}>Open confirmation</Button>
        <Button onPress={() => setFormOpen(true)}>Open form dialog</Button>
        <Button onPress={() => toast.show({ tone: "info", title: "Synthetic export started" })}>
          Information notice
        </Button>
        <Button onPress={() => toast.show({ tone: "success", title: "Label printed" })}>
          Success notice
        </Button>
        <Button
          onPress={() =>
            toast.show({
              tone: "warning",
              title: "Synthetic patient registered on an identity hold",
              description: "Stays until dismissed.",
            })
          }
        >
          Warning notice
        </Button>
        <Button
          onPress={() =>
            toast.show({
              tone: "danger",
              title: "Printer not reachable",
              description: "Stays until dismissed.",
            })
          }
        >
          Danger notice
        </Button>
      </div>
      <Toolbar hint="12 of 340 synthetic records shown.">
        <Button>Open record</Button>
      </Toolbar>
      <Dialog
        title="Dialog title"
        isOpen={dialogOpen}
        onOpenChange={setDialogOpen}
        actions={<Button onPress={() => setDialogOpen(false)}>Close</Button>}
      >
        Dialog content. Escape or Close returns focus to the button that opened it.
      </Dialog>
      {formOpen && (
        <FormDialog
          title="Hold synthetic batch"
          submitLabel="Hold batch"
          form={form}
          onSubmit={form.handleSubmit(() => setFormOpen(false))}
          command={{ error: null, isPending: false }}
          errorTitle="The batch was not held"
          onClose={() => setFormOpen(false)}
        >
          <p>Escape and a click outside close this dialog only while nothing is typed.</p>
          <ReasonField />
          <PasswordField />
        </FormDialog>
      )}
      <ConfirmDialog
        title="Discard synthetic batch?"
        isOpen={confirmOpen}
        onOpenChange={setConfirmOpen}
        confirmLabel="Discard batch"
        onConfirm={() => setConfirmOpen(false)}
      >
        The synthetic batch SYN-000042 will be discarded. This cannot be undone. Escape cancels; a
        click outside does not.
      </ConfirmDialog>
    </GallerySection>
  );
}
