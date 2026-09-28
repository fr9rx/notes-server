import { m } from "motion/react";
import { useEffect, useState } from "react";
import { useNavigate } from "react-router";

import { Page } from "../app/page";
import { Button, ButtonLink } from "../components/Button";
import { LEDMatrix } from "../led/LEDMatrix";
import { spring } from "../motion/springs";

export default function NotFound() {
  const navigate = useNavigate();
  const [intro, setIntro] = useState<string | undefined>("NOT FOUND");
  useEffect(() => {
    document.title = "Not found · notes";
  }, []);
  return (
    <Page className="flex min-h-[80vh] flex-col items-center justify-center px-4 pt-24 text-center">
      <m.div initial={{ scale: 0.94, opacity: 0 }} animate={{ scale: 1, opacity: 1 }} transition={spring.soft}>
        <div className="hidden sm:block">
          <LEDMatrix size="lg" pattern="404" text={intro} onTextDone={() => setIntro(undefined)} label="404" />
        </div>
        <div className="sm:hidden">
          <LEDMatrix size="md" pattern="404" text={intro} onTextDone={() => setIntro(undefined)} label="404" />
        </div>
      </m.div>
      <m.div
        initial={{ opacity: 0, y: 10 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ ...spring.soft, delay: 0.15 }}
        className="mt-10"
      >
        <h1 className="t-display-m text-fg-1">This page isn't on the board.</h1>
        <p className="t-body mt-3 text-fg-2">The link may be old, or the note was removed.</p>
        <div className="mt-8 flex flex-wrap justify-center gap-3">
          <ButtonLink to="/" variant="primary">
            Back to courses
          </ButtonLink>
          <Button variant="secondary" onClick={() => navigate(-1)}>
            Go back
          </Button>
        </div>
      </m.div>
    </Page>
  );
}
