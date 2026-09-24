/**
 * Team Challenges Dashboard Page
 * Issue #1423: Corporate offset goals - team challenges
 */

import { TeamChallengesDashboard } from '@/components/modules/team-challenges/TeamChallengesDashboard';
import { Metadata } from 'next';
import { getServerSession } from 'next-auth';
import { authOptions } from '@/lib/auth';

export const metadata: Metadata = {
  title: 'Team Challenges | Farm-credit',
  description: 'Compete with colleagues on sustainability goals',
};

export default async function TeamChallengesPage() {
  const session = await getServerSession(authOptions);
  
  if (!session?.user?.companyId) {
    return (
      <div className="flex items-center justify-center h-[60vh]">
        <div className="text-center">
          <h2 className="text-xl font-bold mb-2">Access Denied</h2>
          <p className="text-muted-foreground">
            You must be part of a company to access team challenges.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="container mx-auto py-8 px-4">
      <div className="mb-8">
        <h1 className="text-3xl font-bold tracking-tight">Team Challenges</h1>
        <p className="text-muted-foreground mt-2">
          Compete with colleagues on sustainability goals: trees planted, CO₂ offset, sponsorships
        </p>
      </div>
      <TeamChallengesDashboard companyId={session.user.companyId} />
    </div>
  );
}