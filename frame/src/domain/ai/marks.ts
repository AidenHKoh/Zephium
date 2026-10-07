import { ZephiumMark } from "$shared/ui/Icon";
import type { IconSvgElement } from "@hugeicons/svelte";
import type { WorkModelProvider } from "$shared/ipc/bindings";
import ChatGptIcon from "@hugeicons/core-free-icons/ChatGptIcon";
import ClaudeIcon from "@hugeicons/core-free-icons/ClaudeIcon";
import DeepseekIcon from "@hugeicons/core-free-icons/DeepseekIcon";
import GoogleGeminiIcon from "@hugeicons/core-free-icons/GoogleGeminiIcon";
import ServerStack01Icon from "@hugeicons/core-free-icons/ServerStack01Icon";

/** OpenRouter has no Hugeicons glyph: one route forking into two, drawn to its grid. */
const OpenRouterMark: IconSvgElement = [
  ["path", { d: "M3 12H8.5", strokeLinecap: "round", key: "0" }],
  [
    "path",
    {
      d: "M8.5 12C11 12 11.5 7 15 7H20",
      strokeLinecap: "round",
      strokeLinejoin: "round",
      key: "1",
    },
  ],
  [
    "path",
    {
      d: "M8.5 12C11 12 11.5 17 15 17H20",
      strokeLinecap: "round",
      strokeLinejoin: "round",
      key: "2",
    },
  ],
  [
    "path",
    { d: "M17.5 4.5L20 7L17.5 9.5", strokeLinecap: "round", strokeLinejoin: "round", key: "3" },
  ],
  [
    "path",
    { d: "M17.5 14.5L20 17L17.5 19.5", strokeLinecap: "round", strokeLinejoin: "round", key: "4" },
  ],
];

const marks: Record<WorkModelProvider, IconSvgElement> = {
  anthropic: ClaudeIcon,
  open_ai: ChatGptIcon,
  google: GoogleGeminiIcon,
  deep_seek: DeepseekIcon,
  open_router: OpenRouterMark,
  compatible: ServerStack01Icon,
  cloud: ZephiumMark,
};

export function providerMark(provider: WorkModelProvider): IconSvgElement {
  return marks[provider];
}
