/**
 * The project's playbook names for the editor's "Does" choice, loaded once when
 * the editor opens, with why they could not be loaded.
 */
import { useEffect, useState } from 'react';
import { errorMessage } from '@/lib/api-client';
import { listPlaybooks } from '../playbooks/api';

/** The playbook names (empty until loaded) and the load error. */
export function usePlaybookNames(apiKey: string) {
  const [names, setNames] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    listPlaybooks(apiKey)
      .then((list) => setNames(list.playbooks.map((p) => p.name)))
      .catch((err) => setError(errorMessage(err)));
  }, [apiKey]);
  return { names, error };
}
