import React, { useState } from 'react';
import History from './History';

const QuickActions = () => {
  const [isHistoryOpen, setIsHistoryOpen] = useState(false);

  return (
    <>
      <div className="quick-actions">
        <button 
          className="quick-action-btn history-btn"
          onClick={() => setIsHistoryOpen(true)}
          title="Measurement History"
        >
          <span className="btn-text">HISTORY</span>
        </button>
        
        <a 
          className="quick-action-btn docs-btn"
          href="https://specure.github.io/nettest/docs"
          target="_blank"
          rel="noopener noreferrer"
          title="Documentation"
        >
          <span className="btn-text">DOCS</span>
        </a>
      </div>

      <History
        isOpen={isHistoryOpen}
        onClose={() => setIsHistoryOpen(false)}
      />
    </>
  );
};

export default QuickActions; 