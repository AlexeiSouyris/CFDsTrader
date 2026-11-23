"""M1: regresión logística con penalización L2, en PyTorch (docs/spec/ml-v1.md §4).

- Variables estandarizadas con la media y el desvío **solo del entrenamiento** (§8.4). Una
  columna constante en el entrenamiento queda en 0 (desvío 1).
- Ajuste en lote completo con L-BFGS en `float64`, con semilla fija: mismo resultado con los
  mismos datos (§8.5).
- La fuerza de la L2 se elige con el corte interno del entrenamiento (§5), nunca con la
  validación.
"""

from collections.abc import Sequence
from dataclasses import dataclass

import torch

from cfds_intel.models.dataset import Example
from cfds_intel.models.metrics import log_loss
from cfds_intel.models.splits import inner_split

DTYPE = torch.float64


def _matrix(examples: Sequence[Example]) -> torch.Tensor:
    return torch.tensor([e.x for e in examples], dtype=DTYPE)


@dataclass(frozen=True)
class M1:
    mean: torch.Tensor
    std: torch.Tensor
    weights: torch.Tensor
    bias: torch.Tensor
    l2: float

    @classmethod
    def fit(cls, train: Sequence[Example], l2: float, seed: int, max_iterations: int) -> "M1":
        if not train:
            raise ValueError("sin ejemplos de entrenamiento")
        x = _matrix(train)
        y = torch.tensor([float(e.y) for e in train], dtype=DTYPE)
        mean = x.mean(dim=0)
        std = x.std(dim=0, unbiased=False)
        std = torch.where(std > 0, std, torch.ones_like(std))
        z = (x - mean) / std
        torch.manual_seed(seed)
        w = torch.zeros(z.shape[1], dtype=DTYPE, requires_grad=True)
        b = torch.zeros(1, dtype=DTYPE, requires_grad=True)
        opt = torch.optim.LBFGS(
            [w, b], max_iter=max_iterations, line_search_fn="strong_wolfe", tolerance_grad=1e-10
        )

        def closure() -> torch.Tensor:
            opt.zero_grad()
            logits = z @ w + b
            loss = torch.nn.functional.binary_cross_entropy_with_logits(logits, y)
            loss = loss + l2 * (w @ w) / 2
            loss.backward()  # type: ignore[no-untyped-call]
            return loss

        opt.step(closure)  # type: ignore[no-untyped-call]
        return cls(mean, std, w.detach(), b.detach(), l2)

    def predict(self, examples: Sequence[Example]) -> list[float]:
        if not examples:
            return []
        z = (_matrix(examples) - self.mean) / self.std
        return [float(p) for p in torch.sigmoid(z @ self.weights + self.bias)]


def fit_with_inner_tuning(
    train: Sequence[Example],
    l2_grid: Sequence[float],
    tune_fraction: float,
    seed: int,
    max_iterations: int,
) -> M1:
    """Elige la L2 de menor log loss en el corte interno y reentrena con todo el entrenamiento."""
    fit, tune = inner_split(train, tune_fraction)
    best = l2_grid[0]
    if fit and tune:
        losses = []
        for l2 in l2_grid:
            m = M1.fit(fit, l2, seed, max_iterations)
            losses.append(log_loss([e.y for e in tune], m.predict(tune)))
        best = l2_grid[losses.index(min(losses))]
    return M1.fit(train, best, seed, max_iterations)
