'use client';

import { useState } from 'react';
import { ArrowUpRight, Check, Copy, Plug } from 'lucide-react';
import { Button } from '@/components/ui/button';

const repository = 'https://github.com/byalex33/plugget';
const installCommand =
  'cargo install --git https://github.com/byalex33/plugget.git --locked';
const commands = [
  ['search luckperms', 'Find a plugin on Modrinth.'],
  [
    'install luckperms',
    'Install a compatible release and its required dependencies.',
  ],
  ['outdated', 'See which managed plugins have compatible updates.'],
  [
    'update --all',
    'Update your managed plugins with recoverable transactions.',
  ],
];

export default function Home() {
  const [copyState, setCopyState] = useState('');
  async function copyInstall() {
    try {
      await navigator.clipboard.writeText(installCommand);
      setCopyState('Copied. Paste it into your terminal.');
    } catch {
      setCopyState('Select the command to copy it manually.');
    }
  }

  return (
    <>
      <a className="skip-link" href="#main">
        Skip to content
      </a>
      <div className="page">
        <header className="header">
          <a className="wordmark" href="#main" aria-label="Plugget home">
            <Plug strokeWidth={2.4} aria-hidden="true" />
            plugget
          </a>
          <nav aria-label="Main navigation">
            <a href="#commands">Commands</a>
            <a href={`${repository}/blob/main/docs/guide.md`}>
              Docs <ArrowUpRight size={15} />
            </a>
            <a href={repository}>
              GitHub <ArrowUpRight size={15} />
            </a>
          </nav>
        </header>

        <main id="main">
          <section className="intro" aria-labelledby="intro-heading">
            <div className="intro-copy">
              <p className="eyebrow">
                MINECRAFT SERVER TOOLS <span>/</span> EST. 2026
              </p>
              <h1 id="intro-heading">
                Plugin management,
                <br />
                from the
                <br />
                <span>command line.</span>
              </h1>
              <p className="intro-description">
                Search Modrinth. Install compatible releases.
                <br className="wide-only" /> Keep track of what’s on your
                server.
              </p>
              <a className="inline-link" href="#commands">
                See the commands <span aria-hidden="true">↓</span>
              </a>
            </div>

            <aside
              className="install"
              id="install"
              aria-labelledby="install-heading"
            >
              <div className="install-heading">
                <h2 id="install-heading">Install Plugget</h2>
                <span>v0.1.0</span>
              </div>
              <p>
                A native binary, built with Rust.
                <br />
                No server-side plugin required.
              </p>
              <div className="command-label">
                <span>BUILD FROM SOURCE</span>
                <span>CARGO</span>
              </div>
              <div className="install-command">
                <code>{installCommand}</code>
              </div>
              <Button className="copy-button" onClick={copyInstall}>
                {copyState.startsWith('Copied') ? (
                  <Check size={16} />
                ) : (
                  <Copy size={16} />
                )}{' '}
                {copyState.startsWith('Copied')
                  ? 'Copied'
                  : 'Copy install command'}
              </Button>
              <output className="copy-feedback" aria-live="polite">
                {copyState || 'Requires Rust 1.88+ and Cargo on your PATH.'}
              </output>
              <div className="availability">
                <span className="availability-mark" aria-hidden="true">
                  ↳
                </span>
                <p>
                  Early release. Source builds are available now. Prebuilt
                  binaries are coming later.
                </p>
              </div>
              <a
                className="inline-link"
                href={`${repository}/blob/main/README.md#installation`}
              >
                Installation notes <ArrowUpRight size={16} />
              </a>
            </aside>
          </section>

          <div className="compatibility">
            <span className="label">WORKS WITH</span>
            <p>
              Paper <span>/</span> Purpur <span>/</span> Spigot <span>/</span>{' '}
              Bukkit
            </p>
            <span className="compatibility-detail">
              Windows · macOS · Linux source builds
            </span>
          </div>

          <section
            className="commands"
            id="commands"
            aria-labelledby="commands-heading"
          >
            <div className="commands-heading">
              <div>
                <span className="section-index">01 — USAGE</span>
                <h2 id="commands-heading">You already know the workflow.</h2>
              </div>
              <p>
                Run these inside your server directory.
                <br />
                Start with <code>plugget init</code> to detect your setup.
              </p>
            </div>
            <dl className="command-list">
              {commands.map(([command, description]) => (
                <div className="command-row" key={command}>
                  <dt>
                    <span className="prompt" aria-hidden="true">
                      $
                    </span>
                    <code>
                      <span>plugget</span> {command}
                    </code>
                  </dt>
                  <dd>{description}</dd>
                </div>
              ))}
            </dl>
            <div className="commands-bottom">
              <span>
                Also: <code>info</code>, <code>list</code>, <code>remove</code>,{' '}
                <code>doctor</code>
              </span>
              <a href={`${repository}/blob/main/docs/guide.md`}>
                Full command reference <ArrowUpRight size={16} />
              </a>
            </div>
          </section>

          <section className="details" aria-labelledby="details-heading">
            <div className="details-title">
              <span className="section-index">02 — THE DETAILS</span>
              <h2 id="details-heading">
                It’s still
                <br />
                your server.
              </h2>
              <p>
                Plugget manages the JARs it installs.
                <br />
                You stay in charge of the rest.
              </p>
            </div>
            <dl className="detail-list">
              <div>
                <dt>Compatibility before installation</dt>
                <dd>
                  Minecraft versions match upstream metadata exactly. Paper can
                  accept Bukkit and Spigot plugins; the reverse is not assumed.
                  Stable releases are the default.
                </dd>
              </div>
              <div>
                <dt>Downloads are checked</dt>
                <dd>
                  File size, SHA512, and JAR structure are verified before
                  installation. Required dependencies are planned together and
                  confirmed before changes.
                </dd>
              </div>
              <div>
                <dt>Owned files only</dt>
                <dd>
                  Existing, unmanaged JARs remain untouched. Removal sends a
                  managed JAR to the OS Trash and retains its configuration and
                  data.
                </dd>
              </div>
              <div>
                <dt>A record of every change</dt>
                <dd>
                  Updates retain the previous JAR until commit. An exclusive
                  process lock and a recovery journal protect against
                  overlapping or interrupted operations.
                </dd>
              </div>
            </dl>
          </section>

          <div className="operator-note">
            <strong>Before you run it</strong>
            <p>
              Stop your server before changing plugins, then restart it. Use a
              local filesystem with hard-link support and a working Recycle Bin
              or Trash. Keep normal backups; upstream compatibility metadata is
              not a guarantee.
            </p>
          </div>

          <section className="contribute" aria-labelledby="contribute-heading">
            <div>
              <span className="section-index">03 — OPEN SOURCE</span>
              <h2 id="contribute-heading">
                Read the code.
                <br />
                Make it better.
              </h2>
            </div>
            <div className="contribute-copy">
              <p>
                Plugget is MIT licensed. Bug reports, documentation fixes, and
                compatibility test cases are welcome.
              </p>
              <a className="source-link" href={repository}>
                byalex33 / plugget <ArrowUpRight size={20} />
              </a>
              <a className="issue-link" href={`${repository}/issues`}>
                Report an issue <ArrowUpRight size={15} />
              </a>
            </div>
          </section>
        </main>

        <footer>
          <div className="footer-first">
            <a className="wordmark" href="#main">
              plugget
            </a>
            <span>The package manager for Minecraft server plugins.</span>
            <a href={`${repository}/blob/main/LICENSE`}>MIT License ↗</a>
          </div>
          <p>
            Independent project. Not affiliated with Mojang, Microsoft, PaperMC,
            or Modrinth.
          </p>
        </footer>
      </div>
    </>
  );
}
