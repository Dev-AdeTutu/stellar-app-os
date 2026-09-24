/**
 * Team Challenges Service Tests
 * Issue #1423
 */

import { 
  TeamChallengesService, 
  Team, 
  TeamMember, 
  Challenge, 
  ChallengeTeam, 
  ChallengeLeaderboardEntry, 
  ChallengeConfig, 
  ChallengeMetric,
  ChallengeTemplate,
  getMetricLabel,
  getMetricIcon,
  DEFAULT_CHALLENGE_TEMPLATES,
} from './teamChallenges';

describe('TeamChallengesService', () => {
  let service: TeamChallengesService;

  beforeEach(() => {
    service = new TeamChallengesService();
  });

  describe('Team Management', () => {
    it('should create a team', async () => {
      const team = await service.createTeam({
        name: 'Green Warriors',
        description: 'Sustainability champions',
        companyId: 'company-1',
      });
      expect(team).toBeDefined();
      expect(team.name).toBe('Green Warriors');
      expect(team.companyId).toBe('company-1');
      expect(team.members.length).toBeGreaterThanOrEqual(0);
    });

    it('should get company teams', async () => {
      const teams = await service.getCompanyTeams('company-1');
      expect(Array.isArray(teams)).toBe(true);
    });

    it('should get user teams', async () => {
      const teams = await service.getUserTeams('user-1');
      expect(Array.isArray(teams)).toBe(true);
    });
  });

  describe('Challenge Management', () => {
    it('should create a challenge', async () => {
      const challenge = await service.createChallenge({
        name: 'Q1 Sustainability Sprint',
        description: 'Quarterly sustainability challenge',
        companyId: 'company-1',
        startDate: '2024-01-01',
        endDate: '2024-03-31',
        metric: 'offset_per_employee',
        prize: 'Team lunch',
        teamIds: ['team-1', 'team-2'],
      });

      expect(challenge).toBeDefined();
      expect(challenge.name).toBe('Q1 Sustainability Sprint');
      expect(challenge.metric).toBe('offset_per_employee');
      expect(challenge.teams.length).toBe(2);
    });

    it('should reject challenge with less than 2 teams', async () => {
      await expect(service.createChallenge({
        name: 'Test Challenge',
        description: 'Test',
        companyId: 'company-1',
        startDate: '2024-01-01',
        endDate: '2024-03-31',
        metric: 'offset_per_employee',
        teamIds: ['team-1'],
      })).rejects.toThrow('At least 2 teams required');
    });

    it('should reject challenge with invalid dates', async () => {
      await expect(service.createChallenge({
        name: 'Test Challenge',
        description: 'Test',
        companyId: 'company-1',
        startDate: '2024-03-31',
        endDate: '2024-01-01',
        metric: 'offset_per_employee',
        teamIds: ['team-1', 'team-2'],
      })).rejects.toThrow('Start date must be before end date');
    });

    it('should get company challenges', async () => {
      const challenges = await service.getCompanyChallenges('company-1', { status: ['upcoming', 'active'], limit: 10 });
      expect(Array.isArray(challenges)).toBe(true);
    });

    it('should get challenge leaderboard', async () => {
      const leaderboard = await service.getLeaderboard({ metric: 'trees', limit: 10 });
      expect(leaderboard).toBeDefined();
      expect(leaderboard.entries.length).toBeGreaterThan(0);
      expect(leaderboard.metric).toBe('trees');
    });
  });

  describe('Templates', () => {
    it('should return default templates', () => {
      const templates = service.getChallengeTemplates();
      expect(templates.length).toBeGreaterThanOrEqual(4);
    });

    it('should have valid default templates', () => {
      for (const template of DEFAULT_CHALLENGE_TEMPLATES) {
        expect(template.id).toBeTruthy();
        expect(template.name).toBeTruthy();
        expect(template.metric).toBeTruthy();
        expect(template.defaultDurationDays).toBeGreaterThan(0);
      }
    });
  });

  describe('Metric Utilities', () => {
    it('should return correct metric labels', () => {
      expect(getMetricLabel('offset_per_employee')).toBe('Offset per Employee (tons CO₂/employee)');
      expect(getMetricLabel('total_trees')).toBe('Total Trees Planted');
      expect(getMetricLabel('total_co2')).toBe('Total CO₂ Offset (tons)');
    });

    it('should return metric icons', () => {
      expect(getMetricIcon('offset_per_employee')).toBeTruthy();
      expect(getMetricIcon('total_trees')).toBeTruthy();
      expect(getMetricIcon('total_co2')).toBeTruthy();
    });
  });

  describe('Leaderboard Entry Structure', () => {
    it('should have all required fields', async () => {
      const result = await new (require('./teamChallenges').TeamChallengesService)().getLeaderboard({
        metric: 'trees',
        limit: 1,
      });

      const entry = result.entries[0];
      expect(entry).toHaveProperty('rank');
      expect(entry).toHaveProperty('campaignId');
      expect(entry).toHaveProperty('campaignName');
      expect(entry).toHaveProperty('metricValue');
      expect(entry).toHaveProperty('trend');
      expect(['up', 'down', 'same', 'new']).toContain(entry.trend);
    });
  });
});