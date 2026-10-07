import React, { type ReactNode } from 'react';

interface AppViewportFrameProps {
  /** Locks one dense workflow to the visible app frame so its own content can scroll beside fixed actions. */
  viewportBounded?: boolean;
  header?: ReactNode;
  navigation?: ReactNode;
  children: ReactNode;
}

export const AppViewportFrame: React.FC<AppViewportFrameProps> = ({
  viewportBounded = false,
  header,
  navigation,
  children,
}) => (
  <div
    data-testid="application-viewport-frame"
    style={viewportBounded ? { contain: 'layout paint' } : undefined}
    className={`${viewportBounded ? 'h-dvh overflow-hidden' : 'min-h-screen'} flex w-full flex-col bg-slate-100 font-sans text-slate-900`}
  >
    {header && <div className="shrink-0">{header}</div>}
    <div className={`flex min-h-0 flex-1 ${viewportBounded ? 'overflow-hidden' : ''}`}>
      {navigation}
      <main
        data-testid="application-main-content"
        className={`min-w-0 flex-1 px-4 py-4 sm:px-5 sm:py-5 lg:px-6 ${viewportBounded ? 'min-h-0 overflow-hidden' : ''}`}
      >
        <div className={`mx-auto w-full max-w-[1600px] ${viewportBounded ? 'flex h-full min-h-0 flex-col overflow-hidden' : ''}`}>
          {children}
        </div>
      </main>
    </div>
  </div>
);
