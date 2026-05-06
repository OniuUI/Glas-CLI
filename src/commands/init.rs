use std::fs;
use std::path::Path;

use crate::utils;

const INDEX_HTML: &str = r#"<!DOCTYPE html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1.0"><title>Glass House</title><script src="glasshouse/core/glasshouse.js"></script><script src="glasshouse/core/types.js"></script><script src="glasshouse/core/dom.js"></script><script src="glasshouse/blocks/pebble.js"></script><script src="glasshouse/blocks/shine.js"></script><script src="glasshouse/blocks/handler.js"></script><script src="glasshouse/compiler/ts-lexer.js"></script><script src="glasshouse/compiler/ts-parser.js"></script><script src="glasshouse/compiler/ts-binder.js"></script><script src="glasshouse/compiler/ts-checker.js"></script><script src="glasshouse/compiler/ts-emitter.js"></script><script src="glasshouse/compiler/ts-compiler.js"></script><script src="glasshouse/tools/cli.js"></script><script>(function(){"use strict";GlassHouse.ready(function(){GlassHouse.importTS("src/helpers/validation.ts").then(function(){console.log("[TS] validation loaded");return GlassHouse.importTS("src/utilizers/storage.ts")}).then(function(){console.log("[TS] storage loaded");return GlassHouse.importTS("src/shines/glass.shine.ts")}).then(function(){console.log("[TS] glass-shine loaded");return GlassHouse.importTS("src/pebbles/counter.tsx")}).then(function(){console.log("[TS] counter-pebble loaded");return GlassHouse.importTS("src/app.tsx")}).then(function(){console.log("[TS] App loaded \u2014 GlassHouse TSX mode active")}).catch(function(err){console.error("[TS] Load failed:",err)})})})();</script></head><body><div id="app"></div></body></html>"#;

const MAIN_CSS: &str = "*,*::before,*::after{box-sizing:border-box;margin:0;padding:0}html{font-size:16px}body{font-family:system-ui,sans-serif;line-height:1.6;color:#111827;background:#fff;min-height:100vh}#app{min-height:100vh}";

const APP_TSX: &str = r#"'use strict';

GlassHouse.ready(function () {
  var Pebble = GlassHouse.require('pebble');
  var Shine = GlassHouse.require('shine');

  try {
    var shine = GlassHouse.require('glass-shine');
    shine.apply();
  } catch (e) {
    console.log('[App] No shine loaded yet');
  }

  try {
    var counterCreate = GlassHouse.require('counter-pebble').create;
  } catch (e) {}

  var app = new Pebble({
    propTypes: {
      title: 'string'
    },
    stateTypes: {
      count: 'number'
    },
    props: { title: 'Glass House' },
    state: { count: 0 },
    handlers: [],

    render: function () {
      var s = this.state;
      var el = this.el;
      return el('div', { className: 'app' }, [
        el('header', { className: 'header' }, [
          el('h1', {}, [this.props.title || 'Glass House']),
          el('p', { className: 'subtitle' }, ['TSX + Pebble + Shine'])
        ]),
        el('main', { className: 'main' }, [
          el('div', { className: 'card' }, [
            el('h2', {}, ['Counter']),
            el('p', { className: 'count' }, [String(s.count)]),
            el('div', { className: 'actions' }, [
              el('button', { className: 'btn', 'data-action': 'inc' }, ['+']),
              el('button', { className: 'btn', 'data-action': 'dec' }, ['-']),
              el('button', { className: 'btn btn-reset', 'data-action': 'reset' }, ['Reset'])
            ]),
            el('p', { className: 'hint' }, ['Edit src/app.tsx to get started.'])
          ])
        ]),
        el('footer', { className: 'footer' }, [
          el('p', {}, ['Glass House \u2014 zero-dependency framework'])
        ])
      ]);
    },

    delegates: {
      'click [data-action="inc"]': function () {
        this.setState({ count: this.state.count + 1 });
      },
      'click [data-action="dec"]': function () {
        this.setState({ count: this.state.count - 1 });
      },
      'click [data-action="reset"]': function () {
        this.setState({ count: 0 });
      }
    },

    onMount: function () {
      console.log('[App] Glass House ready | TSX mode');
    }
  });

  app.mount('app');
});
"#;

const PEBBLE_TSX: &str = r#"'use strict';

GlassHouse.define('counter-pebble', ['pebble', 'types'], function (Pebble, T) {

  function createCounterPebble(container, props) {
    var merged = Object.assign({}, { label: 'Count', initial: 0 }, props || {});

    var pebble = new Pebble({
      propTypes: {
        label: T.string,
        initial: T.optional(T.number)
      },
      stateTypes: {
        count: T.number
      },
      handlers: ['validation'],

      props: merged,
      state: { count: typeof merged.initial === 'number' ? merged.initial : 0 },

      render: function () {
        var s = this.state;
        var el = this.el;
        return el('div', { className: 'counter' }, [
          el('span', { className: 'counter-label' }, [this.props.label]),
          el('span', { className: 'counter-value' }, [String(s.count)]),
          el('button', { className: 'counter-btn', 'data-action': 'counter-inc' }, ['+']),
          el('button', { className: 'counter-btn', 'data-action': 'counter-dec' }, ['-'])
        ]);
      },

      delegates: {
        'click [data-action="counter-inc"]': function () {
          this.setState({ count: this.state.count + 1 });
        },
        'click [data-action="counter-dec"]': function () {
          if (this.state.count > 0) {
            this.setState({ count: this.state.count - 1 });
          }
        }
      }
    });

    if (container) pebble.mount(container);
    return pebble;
  }

  return { create: createCounterPebble };
});
"#;

const SHINE_TS: &str = r#"'use strict';

GlassHouse.define('glass-shine', ['shine'], function (Shine) {

  var theme = new Shine({
    name: 'glass',
    theme: {
      colors: {
        text: '#111827', background: '#ffffff', primary: '#2563eb',
        secondary: '#7c3aed', muted: '#6b7280', heading: '#1f2937',
        link: '#2563eb', border: '#e5e7eb', buttonText: '#ffffff',
        error: '#dc2626', success: '#16a34a', warning: '#f59e0b', focus: '#2563eb'
      },
      fonts: {
        body: 'system-ui, sans-serif', heading: 'system-ui, sans-serif',
        mono: 'monospace', baseSize: '16px'
      },
      spacing: {
        xs: '0.25rem', sm: '0.5rem', md: '1rem', lg: '1.5rem', xl: '2rem'
      },
      radii: {
        sm: '0.25rem', md: '0.5rem', lg: '0.75rem'
      },
      shadows: {
        sm: '0 1px 2px rgba(0,0,0,0.05)',
        md: '0 4px 6px rgba(0,0,0,0.1)',
        lg: '0 10px 15px rgba(0,0,0,0.1)'
      },
      focus: '#2563eb',
      touchTargets: { minSize: '44px' },
      reduceMotion: true
    },

    onApply: function () {
      var el = document.getElementById('gh-components');
      if (!el) {
        el = document.createElement('style');
        el.id = 'gh-components';
        document.head.appendChild(el);
      }
      el.textContent = [
        ':root,[data-theme="glass"] {',
        '  --clr-text: #111827; --clr-background: #ffffff; --clr-primary: #2563eb;',
        '  --clr-secondary: #7c3aed; --clr-muted: #6b7280; --clr-heading: #1f2937;',
        '  --clr-link: #2563eb; --clr-border: #e5e7eb; --clr-btn-text: #ffffff;',
        '  --clr-error: #dc2626; --clr-success: #16a34a; --clr-warning: #f59e0b;',
        '  --clr-focus: #2563eb;',
        '  --font-body: system-ui, sans-serif; --font-heading: system-ui, sans-serif;',
        '  --font-mono: monospace; --font-base: 16px;',
        '  --space-xs: 0.25rem; --space-sm: 0.5rem; --space-md: 1rem;',
        '  --space-lg: 1.5rem; --space-xl: 2rem;',
        '  --radius-sm: 0.25rem; --radius-md: 0.5rem; --radius-lg: 0.75rem;',
        '  --shadow-sm: 0 1px 2px rgba(0,0,0,0.05);',
        '  --shadow-md: 0 4px 6px rgba(0,0,0,0.1);',
        '  --shadow-lg: 0 10px 15px rgba(0,0,0,0.1);',
        '  --touch-target: 44px;',
        '  color: var(--clr-text); background: var(--clr-background);',
        '  font-family: var(--font-body); font-size: var(--font-base); line-height: 1.6;',
        '}',
        '*,*::before,*::after { box-sizing: border-box; }',
        'body { margin: 0; min-height: 100vh; }',
        '.header { background: var(--clr-heading,#1f2937); color: #ffffff; padding: 2rem 1.5rem; text-align: center; }',
        '.header h1 { font-size: 2rem; font-weight: 700; }',
        '.subtitle { margin-top: 0.5rem; color: var(--clr-muted,#9ca3af); font-size: 0.95rem; }',
        '.main { padding: 2rem 1.5rem; max-width: 720px; margin: 0 auto; }',
        '.card { background: var(--clr-background,#fff); border: 1px solid var(--clr-border,#e5e7eb); border-radius: var(--radius-md,0.5rem); padding: 2rem; margin-bottom: 1.5rem; }',
        '.card h2 { font-size: 1.15rem; font-weight: 600; margin-bottom: 1rem; color: var(--clr-heading,#1f2937); }',
        '.count { font-size: 3rem; font-weight: 700; color: var(--clr-primary,#2563eb); text-align: center; margin: 1rem 0; }',
        '.actions { display: flex; gap: 0.5rem; justify-content: center; }',
        '.btn { background: var(--clr-primary,#2563eb); color: var(--clr-btn-text,#fff); border: 0; padding: 0.5rem 1.25rem; border-radius: var(--radius-sm,0.25rem); font-weight: 500; font-size: 0.9rem; cursor: pointer; min-height: var(--touch-target,44px); min-width: var(--touch-target,44px); }',
        '.btn-reset { background: var(--clr-muted,#6b7280); }',
        '.hint { font-size: 0.8rem; color: var(--clr-muted,#6b7280); margin-top: 0.75rem; text-align: center; }',
        '.footer { background: var(--clr-muted,#6b7280); color: #ffffff; padding: 1rem 1.5rem; text-align: center; font-size: 0.8rem; }',
        ':focus-visible { outline: 2px solid var(--clr-focus,#2563eb); outline-offset: 2px; }',
        '@media (prefers-reduced-motion: reduce) { *,*::before,*::after { animation-duration: 0.01ms!important; transition-duration: 0.01ms!important; } }'
      ].join('\n');
      document.documentElement.setAttribute('data-theme', 'glass');
      console.log('[Shine] Glass theme applied');
    }
  });

  return theme;
});
"#;

const HELPER_TS: &str = r#"'use strict';
GlassHouse.handler('validation', {
  exports: {
    isEmail: { params: ['string'], returns: 'boolean' },
    isRequired: { params: ['string'], returns: 'boolean' }
  },
  factory: function() {
    return {
      isEmail: function(value) {
        return /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value);
      },
      isRequired: function(value) {
        return typeof value === 'string' && value.trim().length > 0;
      }
    };
  }
});
"#;

const UTILIZER_TS: &str = r#"'use strict';
GlassHouse.handler('storage', {
  exports: {
    get: { params: ['string'], returns: 'any' },
    set: { params: ['string', 'any'], returns: 'void' },
    remove: { params: ['string'], returns: 'void' }
  },
  factory: function() {
    return {
      get: function(key) {
        try {
          var raw = localStorage.getItem(key);
          return raw ? JSON.parse(raw) : null;
        } catch (e) {
          return null;
        }
      },
      set: function(key, value) {
        try {
          localStorage.setItem(key, JSON.stringify(value));
        } catch (e) {
          console.error('[storage] Failed to set', key, e);
        }
      },
      remove: function(key) {
        try {
          localStorage.removeItem(key);
        } catch (e) {
          console.error('[storage] Failed to remove', key, e);
        }
      }
    };
  }
});
"#;

// ── GitHub release URLs ──

const GLASSHOUSE_RELEASES: &str = "https://github.com/OniuUI/GlassHouse/releases";

pub fn init(name: &str) {
    init_with_version(name, "latest");
}

pub fn init_with_version(name: &str, glasshouse_version: &str) {
    let root = Path::new(name);
    if root.exists() {
        eprintln!("glas: directory '{}' already exists", name);
        return;
    }
    for d in &["glasshouse", "packages", "src", "styles"] {
        let _ = fs::create_dir_all(root.join(d));
    }
    for (p, c) in &[
        ("index.html", INDEX_HTML),
        ("styles/main.css", MAIN_CSS),
        ("src/app.tsx", APP_TSX),
        ("src/pebbles/counter.tsx", PEBBLE_TSX),
        ("src/shines/glass.shine.ts", SHINE_TS),
        ("src/helpers/validation.ts", HELPER_TS),
        ("src/utilizers/storage.ts", UTILIZER_TS),
        ("packages/.registry.json", "{}"),
        (".gitignore", "dist/\n"),
    ] {
        let fp = root.join(p);
        if let Some(pr) = fp.parent() {
            let _ = fs::create_dir_all(pr);
        }
        let _ = fs::write(&fp, c);
    }
    let gjson = format!(
        r#"{{"name":"{}","version":"0.1.0","entry":"src/app.tsx"}}"#,
        name
    );
    let _ = fs::write(root.join("glass.json"), &gjson);

    let gh_dir = root.join("glasshouse");

    // Cache-first: if no specific version, try local cache
    if glasshouse_version == "latest" {
        if let Some(cache) = utils::find_cached_glasshouse() {
            println!("  Using cached GlassHouse from installer...");
            if let Err(e) = utils::copy_dir_recursive(Path::new(&cache), &gh_dir) {
                eprintln!("glas: warning: cache copy failed: {}. Downloading...", e);
                let _ = fetch_glasshouse(glasshouse_version, &gh_dir);
            } else {
                println!("  GlassHouse (cached) installed.");
            }
        } else {
            let _ = fetch_glasshouse(glasshouse_version, &gh_dir);
        }
    } else {
        let _ = fetch_glasshouse(glasshouse_version, &gh_dir);
    }

    println!("✓ Created Glass House project '{}'", name);
    println!("  cd {}", name);
    println!("  glas dev");
}

fn fetch_glasshouse(version: &str, dest_dir: &Path) -> Result<(), String> {
    if dest_dir.exists() {
        let mut count = 0;
        if let Ok(entries) = fs::read_dir(dest_dir) {
            for _ in entries { count += 1; }
        }
        if count > 0 {
            return Ok(());
        }
    }

    println!("  Fetching GlassHouse {}...", version);

    let real_version = if version == "latest" {
        utils::latest_glasshouse_version()
    } else {
        version.to_string()
    };

    let asset_name = if version == "latest" || version == "nightly" {
        "glasshouse.zip".to_string()
    } else {
        format!("glasshouse-v{}.zip", version)
    };

    let url = format!(
        "{}/download/{}/{}",
        GLASSHOUSE_RELEASES, real_version, asset_name
    );

    let tmp = std::env::temp_dir().join(format!("glasshouse-{}.zip", version));
    utils::fetch_release(&url, &tmp).map_err(|e| {
        let msg = format!("download failed: {}", e);
        eprintln!("glas: {}", msg);
        eprintln!("glas: check available releases with 'glas glasshouse list'");
        msg
    })?;

    utils::extract_zip(&tmp, dest_dir).map_err(|e| {
        let msg = format!("extract failed: {}", e);
        eprintln!("glas: {}", msg);
        let _ = fs::remove_file(&tmp);
        msg
    })?;

    let _ = fs::remove_file(&tmp);

    println!("  GlassHouse {} installed.", real_version);
    Ok(())
}
