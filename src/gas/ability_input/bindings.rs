use crate::ability_system::AbilitySystemComponent;
use crate::gameplay_abilities::AbilitySpecHandle;
use bevy::prelude::Component;
use std::error::Error;
use std::fmt;
use std::mem;

/// Maps logical actions or hotbar slots to abilities granted to one actor.
///
/// Attach this component to the same entity as its [`AbilitySystemComponent`]. Handles are
/// local to that ASC: copying bindings to another actor or replacing the ASC requires rebuilding
/// the bindings, even if numeric handles happen to match. This component does not require an ASC
/// or install input systems automatically.
///
/// Each action has at most one binding; multiple actions may reference the same ability. Entries
/// use a small, contiguous list with linear lookup and deterministic insertion order. Rebinding
/// keeps an action's position; removing entries preserves the order of the remaining actions.
/// No lookup allocates or depends on hash iteration order.
///
/// `Action` is defined by the game, for example an enum containing `PrimaryAttack` and `Slot(u8)`.
/// Physical key mapping, held/released state, and activation policy are managed separately.
#[derive(Component, Debug, Clone)]
pub struct AbilityInputBindings<Action: Send + Sync + 'static> {
    bindings: Vec<(Action, AbilitySpecHandle)>,
}

impl<Action: Send + Sync + 'static> Default for AbilityInputBindings<Action> {
    fn default() -> Self {
        Self {
            bindings: Vec::new(),
        }
    }
}

impl<Action: Send + Sync + 'static> AbilityInputBindings<Action> {
    /// Returns actions and their handles in insertion order, borrowing each action.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Action, AbilitySpecHandle)> {
        self.bindings
            .iter()
            .map(|(action, handle)| (action, *handle))
    }

    /// Returns the number of bound actions, including aliases for the same ability.
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// Returns whether no actions are bound.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }

    /// Removes all bindings while retaining their allocated storage for reuse.
    pub fn clear(&mut self) {
        self.bindings.clear();
    }

    /// Removes every binding to `handle` and returns the number removed.
    ///
    /// Call this after successfully removing an ability from the owning ASC. Removing a binding
    /// does not cancel active abilities or retract already queued activation requests.
    pub fn unbind_ability(&mut self, handle: AbilitySpecHandle) -> usize {
        let previous_len = self.bindings.len();
        self.bindings.retain(|(_, bound)| *bound != handle);
        previous_len - self.bindings.len()
    }
}

impl<Action: Eq + Send + Sync + 'static> AbilityInputBindings<Action> {
    /// Binds `action` to `handle`, returning its previous handle if the action was already bound.
    ///
    /// New actions are appended; rebinding an existing action preserves its iteration position.
    /// The caller must supply a handle granted by the ASC on this entity. Use [`Self::resolve`]
    /// before submitting an activation to check whether the ability is still granted.
    pub fn bind(&mut self, action: Action, handle: AbilitySpecHandle) -> Option<AbilitySpecHandle> {
        if let Some((_, bound)) = self.bindings.iter_mut().find(|(key, _)| *key == action) {
            return Some(mem::replace(bound, handle));
        }
        self.bindings.push((action, handle));
        None
    }

    /// Returns the handle bound to `action`, or `None` if the action is unbound.
    ///
    /// This only reads configuration; use [`Self::resolve`] to also check the owning ASC.
    pub fn get(&self, action: &Action) -> Option<AbilitySpecHandle> {
        self.bindings
            .iter()
            .find(|(key, _)| key == action)
            .map(|(_, handle)| *handle)
    }

    /// Returns the ability bound to `action` if it is still granted by `ability_system`.
    ///
    /// Pass the ASC on the same entity as this component. This checks handle presence,
    /// not entity identity or activation requirements such as cooldown or cost. The resolver validates
    /// the handle and activation requirements again when consuming the queued request.
    ///
    /// # Errors
    ///
    /// Returns [`AbilityInputBindingError::UnboundInput`] when no binding exists, or
    /// [`AbilityInputBindingError::AbilityNotGranted`] when the stored handle no longer exists.
    pub fn resolve(
        &self,
        action: &Action,
        ability_system: &AbilitySystemComponent,
    ) -> Result<AbilitySpecHandle, AbilityInputBindingError> {
        let handle = self
            .get(action)
            .ok_or(AbilityInputBindingError::UnboundInput)?;
        if ability_system.find_ability_spec(handle).is_none() {
            return Err(AbilityInputBindingError::AbilityNotGranted { handle });
        }
        Ok(handle)
    }

    /// Removes the binding for `action`, returning its handle or `None` if it was unbound.
    ///
    /// Remaining entries keep their relative order. Active abilities and queued requests are
    /// unaffected; a later binding of this action will append it at the end of the list.
    pub fn unbind(&mut self, action: &Action) -> Option<AbilitySpecHandle> {
        let index = self.bindings.iter().position(|(key, _)| key == action)?;
        Some(self.bindings.remove(index).1)
    }
}

/// Explains why a logical input could not resolve to a currently granted ability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbilityInputBindingError {
    /// The requested action has no binding.
    UnboundInput,
    /// The action is bound, but its handle is absent from the supplied ASC.
    AbilityNotGranted {
        /// The stale or invalid handle stored in the binding.
        handle: AbilitySpecHandle,
    },
}

impl fmt::Display for AbilityInputBindingError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnboundInput => formatter.write_str("the input action is not bound"),
            Self::AbilityNotGranted { handle } => write!(
                formatter,
                "the bound ability handle {} is not granted by this ASC",
                handle.get_value()
            ),
        }
    }
}

impl Error for AbilityInputBindingError {}
