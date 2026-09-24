/**
 * Project Comparison Page
 * Issue #1416: Project comparison tool - side-by-side review
 */

import { ProjectComparisonTool } from '@/components/modules/project-comparison/ProjectComparisonTool';
import { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Project Comparison | Farm-credit',
  description: 'Compare carbon offset projects side-by-side: price, methodology, verifier, risk, co-benefits, reviews',
};

export default function ProjectComparisonPage() {
  return (
    <div className="container mx-auto py-8 px-4">
      <div className="mb-8">
        <h1 className="text-3xl font-bold tracking-tight">Project Comparison</h1>
        <p className="text-muted-foreground mt-2">
          Compare carbon offset projects side-by-side: price, methodology, verifier, risk, co-benefits, reviews
        </p>
      </div>
      <ProjectComparisonTool />
    </div>
  );
}