import './index.css';
import { createRoot } from 'react-dom/client';
import { ReasoningTrace } from './components/assistant-ui/elements/reasoning-trace';

const TEXT = `**Reading the request**
The user wants the reasoning boxes to use assistant-ui's static element.

**Planning the change**
- vendor \`reasoning-panel\` from the registry
- record per-block timing in the core

**Checking the edge cases**
Legacy rows have no timing, so they settle to a plain label.`;
const now = Date.now();
function App() {
  return (
    <div className="mx-auto flex max-w-xl flex-col gap-8 p-8 text-sm">
      <section><h3 className="mb-2 text-xs opacity-50">streaming (collapsible)</h3>
        <ReasoningTrace texts={[TEXT]} timings={[{ startedAt: now - 7000, endedAt: now }]} streaming /></section>
      <section><h3 className="mb-2 text-xs opacity-50">settled, collapsed</h3>
        <ReasoningTrace texts={[TEXT]} timings={[{ startedAt: now - 23000, endedAt: now - 11000 }]} streaming={false} /></section>
      <section><h3 className="mb-2 text-xs opacity-50">settled, opened</h3>
        <ReasoningTrace texts={[TEXT]} timings={[{ startedAt: now - 23000, endedAt: now - 11000 }]} streaming={false} defaultOpen /></section>
      <section><h3 className="mb-2 text-xs opacity-50">non-collapsible (rail)</h3>
        <ReasoningTrace texts={[TEXT]} timings={[{ startedAt: now - 75000, endedAt: now }]} streaming={false} collapsible={false} /></section>
    </div>
  );
}
createRoot(document.getElementById('root')!).render(<App />);
