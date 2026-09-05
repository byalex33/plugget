'use client';

import { useState } from 'react';
import {
  ArrowUpRight,
  ArrowRight,
  Check,
  Copy,
  CodeXml,
  Plug,
  ShieldCheck,
  GitBranch,
  Terminal,
  Package,
  Command,
} from 'lucide-react';
import { Button } from '@/components/ui/button';

const repository = 'https://github.com/byalex33/plugget';
const installCommand =
  'cargo install --git https://github.com/byalex33/plugget.git --locked';

export default function Home() {
  const [copyState, setCopyState] = useState('');
  async function copyInstall() {
    try {
      await navigator.clipboard.writeText(installCommand);
      setCopyState('Copied to clipboard');
    } catch {
      setCopyState('Select the command below to copy it manually.');
    }
  }
  return (
    <>
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <header className="header wrap">
        <a className="brand" href="#main" aria-label="Plugget home">
          <span className="brand-icon">
            <Plug size={23} strokeWidth={2.5} />
          </span>
          plugget<span className="brand-dot">.</span>
        </a>
        <nav aria-label="Main navigation">
          <a href="#how-it-works">How it works</a>
          <a href={`${repository}/blob/main/docs/guide.md`}>
            Documentation <ArrowUpRight size={14} />
          </a>
          <a className="nav-github" href={repository}>
            <CodeXml size={17} /> GitHub <ArrowUpRight size={14} />
          </a>
        </nav>
      </header>
      <main id="main">
        <section className="hero wrap" aria-labelledby="hero-title">
          <a className="release-label" href={`${repository}/releases`}>
            <span className="status-dot" /> INTRODUCING PLUGGET{' '}
            <span className="version">v0.1.0</span>
            <ArrowRight size={14} />
          </a>
          <h1 id="hero-title">
            Less managing.
            <br />
            <span>More Minecraft.</span>
          </h1>
          <p className="hero-description">
            The open-source package manager for server plugins.
            <br className="desktop-break" /> Find, install, and update your
            plugins. Right from your terminal.
          </p>
          <div className="hero-actions">
            <a className="action primary" href="#install">
              Get Plugget <ArrowRight size={18} />
            </a>
            <a className="action secondary" href={repository}>
              <CodeXml size={18} /> View on GitHub
            </a>
          </div>
          <p className="hero-meta">
            <span>Free &amp; open source</span>
            <i /> MIT licensed <i /> Built with Rust
          </p>
          <div
            className="terminal-window"
            aria-label="Example Plugget installation workflow"
          >
            <div className="terminal-title">
              <div className="window-dots" aria-hidden="true">
                <i />
                <i />
                <i />
              </div>
              <span>
                <Terminal size={13} /> ~/minecraft-server
              </span>
              <span className="terminal-shell">zsh</span>
            </div>
            <div className="terminal-content">
              <div className="terminal-comment">
                # Your next plugin is one command away.
              </div>
              <div className="terminal-command">
                <span>❯</span> plugget install luckperms --yes
              </div>
              <div className="terminal-output">
                <span>
                  <Check size={15} /> Installed LuckPerms v5.5.71-bukkit.
                </span>
                <span className="muted">
                  Restart the server to apply changes.
                </span>
              </div>
              <div className="terminal-command last-command">
                <span>❯</span> plugget{' '}
                <span className="cursor" aria-hidden="true" />
              </div>
            </div>
            <div className="terminal-footer">
              <span>
                <ShieldCheck size={14} /> SHA512 verified
              </span>
              <span>
                Example workflow <span className="small-dot" />
              </span>
            </div>
          </div>
          <div className="platforms">
            <p>AT HOME ON YOUR SERVER</p>
            <div>
              <span>
                <Package /> Paper
              </span>
              <span>
                <Package /> Purpur
              </span>
              <span>
                <Package /> Spigot
              </span>
              <span>
                <Package /> Bukkit
              </span>
            </div>
          </div>
        </section>
        <section
          className="features wrap"
          id="how-it-works"
          aria-labelledby="features-title"
        >
          <div className="section-heading">
            <span className="eyebrow">A SMALL TOOL. A BETTER WORKFLOW.</span>
            <h2 id="features-title">Your plugins, handled.</h2>
            <p>
              All the familiar package-manager essentials.
              <br />
              Built around the way you run a Minecraft server.
            </p>
          </div>
          <div className="feature-grid">
            <article>
              <div className="feature-icon">
                <Command size={23} />
              </div>
              <span className="feature-number">01 / DISCOVER</span>
              <h3>
                The right plugin.
                <br />
                The right version.
              </h3>
              <p>
                Search Modrinth and find a release that matches your Minecraft
                version and server platform. Stable by default.
              </p>
              <code>
                <span>$</span> plugget search luckperms
              </code>
            </article>
            <article>
              <div className="feature-icon">
                <ShieldCheck size={23} />
              </div>
              <span className="feature-number">02 / INSTALL</span>
              <h3>
                Checks first.
                <br />
                Changes second.
              </h3>
              <p>
                Downloads are verified before installation. Required
                dependencies are planned together, and your unmanaged JARs stay
                untouched.
              </p>
              <code>
                <span>$</span> plugget install luckperms
              </code>
            </article>
            <article>
              <div className="feature-icon">
                <GitBranch size={23} />
              </div>
              <span className="feature-number">03 / MAINTAIN</span>
              <h3>
                Keep up.
                <br />
                With a way back.
              </h3>
              <p>
                Check compatible updates and apply them with recoverable
                transactions. Old JARs are retained until the change commits.
              </p>
              <code>
                <span>$</span> plugget update --all
              </code>
            </article>
          </div>
        </section>
        <section
          className="install-section"
          id="install"
          aria-labelledby="install-title"
        >
          <div className="wrap install-layout">
            <div>
              <span className="eyebrow">MEET YOUR NEW SERVER ESSENTIAL</span>
              <h2 id="install-title">
                One install.
                <br />
                Less busywork.
              </h2>
              <p>
                Build Plugget from source with Rust 1.88 or newer.
                <br />
                Then head into your server directory and get going.
              </p>
              <a
                className="text-link"
                href={`${repository}/blob/main/README.md#installation`}
              >
                Read the installation guide <ArrowUpRight size={17} />
              </a>
            </div>
            <div className="install-panel">
              <div className="install-panel-top">
                <span>
                  <Terminal size={16} /> Install from source
                </span>
                <span className="source-badge">RUST / CARGO</span>
              </div>
              <div className="copy-command">
                <code>{installCommand}</code>
                <Button
                  variant="ghost"
                  size="icon"
                  onClick={copyInstall}
                  aria-label="Copy installation command"
                >
                  {copyState === 'Copied to clipboard' ? <Check /> : <Copy />}
                </Button>
              </div>
              <output className="copy-feedback" aria-live="polite">
                {copyState || 'Windows, macOS, and Linux source builds.'}
              </output>
              <div className="next-steps">
                <span className="terminal-comment">
                  # Next, inside your server directory
                </span>
                <code>
                  plugget init
                  <br />
                  plugget install luckperms
                </code>
              </div>
              <p className="install-note">
                Early release. Prebuilt binaries are not published yet.
                <br />
                Stop your server before changes; restart it afterward.
              </p>
            </div>
          </div>
        </section>
        <section className="open-source wrap">
          <div className="oss-mark">
            <CodeXml size={28} />
          </div>
          <div>
            <span className="eyebrow">OPEN SOURCE. OPEN DOORS.</span>
            <h2>
              Built for your server.
              <br />
              Better with your input.
            </h2>
            <p>
              Found a bug? Have a better idea? Help shape a small, focused tool
              <br className="desktop-break" /> for the Minecraft server
              community. Every contribution counts.
            </p>
            <a className="action secondary" href={repository}>
              Explore the source <ArrowUpRight size={17} />
            </a>
          </div>
        </section>
      </main>
      <footer className="footer wrap">
        <div className="footer-top">
          <a className="brand" href="#main">
            <span className="brand-icon">
              <Plug size={19} />
            </span>
            plugget<span className="brand-dot">.</span>
          </a>
          <span className="footer-tagline">
            Your server. Your plugins. Your terminal.
          </span>
          <div>
            <a href={`${repository}/blob/main/docs/guide.md`}>Docs</a>
            <a href={`${repository}/issues`}>Issues</a>
            <a href={`${repository}/blob/main/LICENSE`}>MIT License</a>
          </div>
        </div>
        <div className="footer-bottom">
          <span>Made for the Minecraft community.</span>
          <span>
            Not affiliated with Mojang, Microsoft, PaperMC, or Modrinth.
          </span>
        </div>
      </footer>
    </>
  );
}
