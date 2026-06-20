import { describe, expect, it } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { App } from "../App";
import { useStore } from "../store";
import { defaultConfig } from "../types";

describe("DupHunter UI smoke test", () => {
  it("renders the setup screen with the offline badge", () => {
    useStore.setState({ view: "setup", config: defaultConfig() });
    render(<App />);
    expect(screen.getByText("DupHunter")).toBeInTheDocument();
    expect(screen.getByText(/nothing leaves this PC/i)).toBeInTheDocument();
    expect(screen.getByText(/Folders to scan/i)).toBeInTheDocument();
  });

  it("switches detection mode to video near-dup", () => {
    useStore.setState({ view: "setup", config: defaultConfig() });
    render(<App />);
    const videoRadio = screen.getByLabelText(/Video near-duplicates/i);
    fireEvent.click(videoRadio);
    expect(useStore.getState().config.mode).toBe("video-near-dup");
  });

  it("computes reclaimable space from current selection", () => {
    useStore.setState({
      view: "results",
      sets: [
        {
          id: 0,
          mode: "exact",
          hash: "h",
          similarity: null,
          reclaimable: 100,
          zeroByte: false,
          members: [
            {
              role: "keep",
              isHardlinkOfOther: false,
              entry: { path: "/a", root: "/", size: 100, mtime: 0, identity: null },
            },
            {
              role: "remove",
              isHardlinkOfOther: false,
              entry: { path: "/b", root: "/", size: 100, mtime: 0, identity: null },
            },
          ],
        },
      ],
    });
    expect(useStore.getState().reclaimableSelected()).toBe(100);
    expect(useStore.getState().removalCount()).toBe(1);
  });
});
